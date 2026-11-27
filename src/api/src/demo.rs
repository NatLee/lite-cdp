//! 範例資料：兩個網站、三個帳號，和最近 30 天的訪客與事件。
//!
//! 事件是用 [`ingest::store_in`] 一筆一筆「重播」進去的，所以認人、合併、
//! 標籤累計都走和正式收資料一樣的路。亂數是固定種子，每次產生的人和行為都一樣，
//! 只有時間是從現在往回推。

use std::collections::HashSet;

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Map, Value};
use sqlx::{Sqlite, Transaction};

use crate::{
    auth, db,
    ingest::{self, Batch, Event, Identify},
    segments::{Condition, Rules},
    state::AppState,
};

pub const MEMBER_USERNAMES: [&str; 2] = ["amy", "ben"];
const DAYS: u64 = 30;

pub struct Summary {
    pub users: usize,
    pub sites: usize,
    pub people: i64,
    pub events: i64,
}

/// 線性同餘亂數：夠用，而且每次結果都一樣。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    fn weighted<'a, T>(&mut self, items: &'a [T], weight: impl Fn(&T) -> u64) -> &'a T {
        let total: u64 = items.iter().map(&weight).sum();
        let mut roll = self.below(total);
        for item in items {
            let w = weight(item);
            if roll < w {
                return item;
            }
            roll -= w;
        }
        &items[items.len() - 1]
    }
}

struct Page {
    path: &'static str,
    title: &'static str,
    weight: u64,
}

/// 看到某一頁時，有 `percent`% 的機率做這件事。
struct Action {
    path: &'static str,
    kind: &'static str,
    name: &'static str,
    percent: u64,
}

struct LabelSpec {
    name: &'static str,
    color: &'static str,
    description: &'static str,
    min_hits: i64,
    rules: &'static [(&'static str, &'static str)],
}

struct Blueprint {
    name: &'static str,
    url: &'static str,
    members: &'static [&'static str],
    pages: &'static [Page],
    actions: &'static [Action],
    labels: &'static [LabelSpec],
    /// 認出身分的那一頁，和當下觸發的事件
    identify_page: &'static str,
    identify_event: &'static str,
    identified: u64,
    anonymous: u64,
    trait_sets: &'static [(&'static str, &'static [&'static str])],
}

const fn page(path: &'static str, title: &'static str, weight: u64) -> Page {
    Page {
        path,
        title,
        weight,
    }
}

const fn action(
    path: &'static str,
    kind: &'static str,
    name: &'static str,
    percent: u64,
) -> Action {
    Action {
        path,
        kind,
        name,
        percent,
    }
}

const BRAND: Blueprint = Blueprint {
    name: "品牌官網",
    url: "https://www.example.com",
    members: &["amy"],
    pages: &[
        page("/", "首頁｜範例雲端", 30),
        page("/features", "功能介紹｜範例雲端", 16),
        page("/features/automation", "自動化流程｜範例雲端", 7),
        page("/pricing", "方案與價格｜範例雲端", 15),
        page("/enterprise", "企業方案｜範例雲端", 6),
        page("/blog", "部落格｜範例雲端", 10),
        page("/blog/cdp-101", "什麼是 CDP？｜部落格", 9),
        page("/blog/first-party-data", "第一方資料入門｜部落格", 7),
        page("/docs/getting-started", "快速開始｜說明文件", 6),
        page("/contact", "聯絡我們｜範例雲端", 5),
        page("/about", "關於我們｜範例雲端", 3),
    ],
    actions: &[
        action("/", "click", "免費試用", 14),
        action("/pricing", "click", "開始免費試用", 22),
        action("/enterprise", "click", "預約展示", 25),
        action("/contact", "custom", "contact_submit", 30),
        action("/blog/cdp-101", "click", "訂閱電子報", 10),
    ],
    labels: &[
        LabelSpec {
            name: "看過定價",
            color: "amber",
            description: "看過方案與價格頁",
            min_hits: 1,
            rules: &[("path_prefix", "/pricing")],
        },
        LabelSpec {
            name: "部落格讀者",
            color: "sky",
            description: "看過兩篇以上的部落格文章",
            min_hits: 2,
            rules: &[("path_prefix", "/blog")],
        },
        LabelSpec {
            name: "對企業方案有興趣",
            color: "violet",
            description: "看過企業方案，或按了預約展示",
            min_hits: 1,
            rules: &[("path_prefix", "/enterprise"), ("event", "預約展示")],
        },
        LabelSpec {
            name: "已註冊試用",
            color: "green",
            description: "送出過註冊表單",
            min_hits: 1,
            rules: &[("event", "signup")],
        },
        LabelSpec {
            name: "想聯絡業務",
            color: "red",
            description: "送出過聯絡表單",
            min_hits: 1,
            rules: &[("event", "contact_submit")],
        },
    ],
    identify_page: "/signup",
    identify_event: "signup",
    identified: 64,
    anonymous: 210,
    trait_sets: &[
        ("方案", &["免費版", "免費版", "團隊版", "團隊版", "企業版"]),
        (
            "公司",
            &["範例科技", "示範商行", "樣本設計", "假想物流", "測試餐飲"],
        ),
        ("職稱", &["行銷企劃", "產品經理", "工程師", "業務", "老闆"]),
    ],
};

const SHOP: Blueprint = Blueprint {
    name: "線上商店",
    url: "https://shop.example.com",
    members: &["amy", "ben"],
    pages: &[
        page("/", "範例咖啡｜線上商店", 26),
        page("/products", "所有商品｜範例咖啡", 18),
        page("/products/coffee-beans", "莊園咖啡豆｜範例咖啡", 14),
        page("/products/drip-bag", "濾掛咖啡禮盒｜範例咖啡", 11),
        page("/products/mug", "陶瓷馬克杯｜範例咖啡", 6),
        page("/sale", "週年慶特價｜範例咖啡", 10),
        page("/cart", "購物車｜範例咖啡", 8),
        page("/checkout", "結帳｜範例咖啡", 4),
        page("/stores", "門市據點｜範例咖啡", 3),
    ],
    actions: &[
        action("/products/coffee-beans", "custom", "add_to_cart", 30),
        action("/products/drip-bag", "custom", "add_to_cart", 26),
        action("/products/mug", "custom", "add_to_cart", 18),
        action("/sale", "click", "領取折價券", 20),
        action("/checkout", "custom", "purchase", 55),
    ],
    labels: &[
        LabelSpec {
            name: "加入購物車",
            color: "amber",
            description: "把商品放進過購物車",
            min_hits: 1,
            rules: &[("event", "add_to_cart")],
        },
        LabelSpec {
            name: "已下單",
            color: "green",
            description: "完成過結帳",
            min_hits: 1,
            rules: &[("event", "purchase")],
        },
        LabelSpec {
            name: "咖啡豆愛好者",
            color: "brand",
            description: "看了三次以上咖啡豆",
            min_hits: 3,
            rules: &[("path_prefix", "/products/coffee-beans")],
        },
        LabelSpec {
            name: "逛特價區",
            color: "pink",
            description: "看過週年慶特價頁",
            min_hits: 1,
            rules: &[("path_prefix", "/sale"), ("title_contains", "特價")],
        },
    ],
    identify_page: "/account",
    identify_event: "login",
    identified: 48,
    anonymous: 150,
    trait_sets: &[
        ("會員等級", &["一般", "一般", "一般", "銀卡", "金卡"]),
        ("城市", &["台北", "新北", "台中", "台南", "高雄"]),
    ],
};

/// 從哪裡來的：（來源網址、比重）。空字串是直接輸入網址。
const REFERRERS: [(&str, u64); 7] = [
    ("", 38),
    ("https://www.google.com/", 26),
    ("https://www.facebook.com/", 12),
    ("https://www.linkedin.com/feed/", 6),
    ("https://t.co/demo", 5),
    ("https://news.example.org/tech/review", 7),
    ("https://forum.example.net/thread/42", 6),
];

const USER_AGENTS: [&str; 5] = [
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0",
];

const SURNAMES: [(&str, &str); 12] = [
    ("林", "lin"),
    ("陳", "chen"),
    ("黃", "huang"),
    ("張", "chang"),
    ("李", "lee"),
    ("王", "wang"),
    ("吳", "wu"),
    ("劉", "liu"),
    ("蔡", "tsai"),
    ("楊", "yang"),
    ("許", "hsu"),
    ("鄭", "cheng"),
];

const GIVEN_NAMES: [(&str, &str); 14] = [
    ("雅婷", "yating"),
    ("怡君", "yichun"),
    ("家豪", "chiahao"),
    ("志明", "chihming"),
    ("淑芬", "shufen"),
    ("冠宇", "kuanyu"),
    ("詩涵", "shihhan"),
    ("承恩", "chengen"),
    ("宜蓁", "yichen"),
    ("柏翰", "pohan"),
    ("欣怡", "hsinyi"),
    ("俊傑", "chunchieh"),
    ("佩珊", "peishan"),
    ("宗翰", "tsunghan"),
];

const DOMAINS: [&str; 3] = ["example.com", "example.org", "example.net"];

/// 一個要被重播的人。
struct Visitor {
    anon_id: String,
    user_agent: &'static str,
    identify: Option<Identify>,
    /// 第一次來是第幾天（0 是 30 天前，29 是今天）
    first_day: u64,
    sessions: u64,
    /// 只在一開始那幾天來過，之後就沒再出現
    dormant: bool,
}

fn event(site: &Blueprint, kind: &'static str, name: &str, page: &Page, referrer: &str) -> Event {
    let url = format!("{}{}", site.url, page.path);
    let host = site.url.trim_start_matches("https://");
    Event {
        event_id: String::new(),
        happened_at: None,
        kind,
        name: name.to_owned(),
        path: page.path.to_owned(),
        title: page.title.to_owned(),
        referrer_host: ingest::referrer_host(referrer, host),
        referrer: referrer.to_owned(),
        props: "{}".to_owned(),
        url,
    }
}

/// 重播一個人的所有造訪。
async fn replay(
    tx: &mut Transaction<'_, Sqlite>,
    rng: &mut Rng,
    site: &Blueprint,
    site_id: i64,
    visitor: &Visitor,
    now: DateTime<Utc>,
) -> Result<()> {
    let identify_page = Page {
        path: site.identify_page,
        title: if site.identify_page == "/signup" {
            "免費試用｜範例雲端"
        } else {
            "會員中心｜範例咖啡"
        },
        weight: 0,
    };
    // 每一次造訪是第幾天
    let last_day = if visitor.dormant {
        (visitor.first_day + 4).min(DAYS - 1)
    } else {
        DAYS - 1
    };
    let mut days: Vec<u64> = (1..visitor.sessions)
        .map(|_| visitor.first_day + rng.below(last_day - visitor.first_day + 1))
        .collect();
    days.push(visitor.first_day);
    days.sort_unstable();
    // 在第幾次造訪時被認出來
    let identify_at = rng.below(visitor.sessions.min(2));

    let mut previous = now - Duration::days(DAYS as i64 + 1);
    for (index, day) in days.iter().enumerate() {
        let days_ago = (DAYS - 1 - day) as i64;
        let mut at = now - Duration::days(days_ago) - Duration::minutes(rng.below(600) as i64 + 2);
        // 同一天來兩次的話，後面那次排在前一次之後
        if at <= previous {
            at = previous + Duration::minutes(20 + rng.below(90) as i64);
        }
        if at >= now {
            at = now - Duration::seconds(30);
        }
        let session_id = format!("{}s{index}", visitor.anon_id);
        let mut referrer = rng.weighted(&REFERRERS, |r| r.1).0.to_owned();
        let pageviews = 1 + rng.below(5);
        for step in 0..=pageviews {
            let identifying =
                step == pageviews && index as u64 == identify_at && visitor.identify.is_some();
            if step == pageviews && !identifying {
                break;
            }
            let page = if identifying {
                &identify_page
            } else if step == 0 && rng.chance(45) {
                &site.pages[0]
            } else {
                rng.weighted(site.pages, |p| p.weight)
            };
            let mut events = vec![event(site, "pageview", "", page, &referrer)];
            if rng.chance(55) {
                events.push(Event {
                    props: json!({ "depth": 50 }).to_string(),
                    ..event(site, "scroll", "50%", page, "")
                });
                if rng.chance(40) {
                    events.push(Event {
                        props: json!({ "depth": 100 }).to_string(),
                        ..event(site, "scroll", "100%", page, "")
                    });
                }
            }
            for act in site.actions.iter().filter(|a| a.path == page.path) {
                if rng.chance(act.percent) {
                    events.push(event(site, act.kind, act.name, page, ""));
                }
            }
            if identifying {
                events.push(event(site, "custom", site.identify_event, page, ""));
            }
            let batch = Batch {
                identity_context: None,
                anon_id: visitor.anon_id.clone(),
                session_id: session_id.clone(),
                events,
                identify: if identifying {
                    visitor.identify.clone()
                } else {
                    None
                },
            };
            let stamp = db::format_time(at.min(now));
            ingest::store_in(tx, site_id, &batch, visitor.user_agent, &stamp).await?;
            // 站內換頁：下一頁的來源是這一頁
            referrer = format!("{}{}", site.url, page.path);
            at += Duration::seconds(15 + rng.below(100) as i64);
        }
        previous = at;
    }
    Ok(())
}

async fn seed_site(
    tx: &mut Transaction<'_, Sqlite>,
    rng: &mut Rng,
    site: &Blueprint,
    emails: &mut HashSet<String>,
    now: DateTime<Utc>,
) -> Result<i64> {
    let created = db::format_time(now - Duration::days(DAYS as i64 + 3));
    let site_id = sqlx::query(
        "INSERT INTO sites (name, url, key, allowed_origins, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(site.name)
    .bind(site.url)
    .bind(auth::random_hex(12))
    .bind(json!([site.url]).to_string())
    .bind(&created)
    .execute(&mut **tx)
    .await?
    .last_insert_rowid();
    for username in site.members {
        sqlx::query(
            "INSERT INTO site_members (site_id, user_id) SELECT ?, id FROM users WHERE username = ?",
        )
        .bind(site_id)
        .bind(username)
        .execute(&mut **tx)
        .await?;
    }
    // 標籤要先有，重播事件時才會跟著累計
    for label in site.labels {
        let label_id = sqlx::query(
            "INSERT INTO labels (site_id, name, color, description, min_hits, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(site_id)
        .bind(label.name)
        .bind(label.color)
        .bind(label.description)
        .bind(label.min_hits)
        .bind(&created)
        .execute(&mut **tx)
        .await?
        .last_insert_rowid();
        for (kind, value) in label.rules {
            sqlx::query("INSERT INTO label_rules (label_id, kind, value) VALUES (?, ?, ?)")
                .bind(label_id)
                .bind(kind)
                .bind(value)
                .execute(&mut **tx)
                .await?;
        }
    }

    let total = site.identified + site.anonymous;
    for index in 0..total {
        let identify = if index < site.identified {
            let (surname, surname_en) = rng.pick(&SURNAMES);
            let (given, given_en) = rng.pick(&GIVEN_NAMES);
            let domain = rng.pick(&DOMAINS);
            let mut email = format!("{given_en}.{surname_en}@{domain}");
            let mut n = 2;
            while !emails.insert(email.clone()) {
                email = format!("{given_en}.{surname_en}{n}@{domain}");
                n += 1;
            }
            let mut traits = Map::new();
            for (key, values) in site.trait_sets {
                traits.insert((*key).to_owned(), Value::from(*rng.pick(values)));
            }
            Some(Identify {
                mutation_id: String::new(),
                revision: None,
                user_id: String::new(),
                verified: false,
                email,
                name: Some(format!("{surname}{given}")),
                account: Some(format!("M-{:04}", 1000 + index)),
                traits,
            })
        } else {
            None
        };
        let known = identify.is_some();
        // 兩次取大的：越靠近今天，新來的人越多
        let first_day = rng.below(DAYS).max(rng.below(DAYS));
        let dormant = known && rng.chance(22);
        let visitor = Visitor {
            anon_id: format!("demo{site_id}v{index:05}"),
            user_agent: rng.pick(&USER_AGENTS),
            first_day: if dormant { rng.below(10) } else { first_day },
            sessions: if known {
                2 + rng.below(6)
            } else {
                1 + rng.below(3)
            },
            dormant,
            identify,
        };
        replay(tx, rng, site, site_id, &visitor, now).await?;

        // 有些會員後來換了一台裝置登入：兩邊的紀錄會併成同一個人
        if known && !dormant && rng.chance(15) {
            let second = Visitor {
                anon_id: format!("demo{site_id}w{index:05}"),
                user_agent: USER_AGENTS[2],
                first_day: (visitor.first_day + 1 + rng.below(5)).min(DAYS - 1),
                sessions: 1 + rng.below(2),
                dormant: false,
                identify: visitor.identify.clone(),
            };
            replay(tx, rng, site, site_id, &second, now).await?;
        }
    }
    Ok(site_id)
}

async fn label_id(tx: &mut Transaction<'_, Sqlite>, site_id: i64, name: &str) -> Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT id FROM labels WHERE site_id = ? AND name = ?")
            .bind(site_id)
            .bind(name)
            .fetch_one(&mut **tx)
            .await?,
    )
}

struct SegmentSpec<'a> {
    name: &'a str,
    description: &'a str,
    kind: &'a str,
    rules: Rules,
}

async fn add_segment(
    tx: &mut Transaction<'_, Sqlite>,
    site_id: i64,
    owner: i64,
    segment: SegmentSpec<'_>,
    now: DateTime<Utc>,
) -> Result<()> {
    let created = db::format_time(now - Duration::days(9));
    sqlx::query(
        "INSERT INTO segments (site_id, name, description, kind, rules, created_by, created_at,
            updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(site_id)
    .bind(segment.name)
    .bind(segment.description)
    .bind(segment.kind)
    .bind(serde_json::to_string(&segment.rules)?)
    .bind(owner)
    .bind(&created)
    .bind(&created)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 在空的資料庫裡建立範例資料；已經有帳號的話不動它。
pub async fn seed(state: &AppState, password: &str) -> Result<Summary> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    if users > 0 {
        bail!("資料庫裡已經有帳號了；範例資料只能建在空的資料庫裡");
    }
    let accounts = [
        ("demo", "Demo 管理員", "admin"),
        ("amy", "Amy（行銷）", "member"),
        ("ben", "Ben（客服）", "member"),
    ];
    let mut owner = 0;
    for (username, display_name, role) in accounts {
        let id = auth::create_user(&state.db, username, display_name, password, role)
            .await
            .map_err(|e| anyhow::anyhow!("建立範例帳號失敗：{e:?}"))?;
        if username == "amy" {
            owner = id;
        }
    }

    let now = Utc::now();
    let mut rng = Rng(20260101);
    let mut emails = HashSet::new();
    let mut tx = state.db.begin().await?;

    let brand = seed_site(&mut tx, &mut rng, &BRAND, &mut emails, now).await?;
    let pricing = label_id(&mut tx, brand, "看過定價").await?;
    let enterprise = label_id(&mut tx, brand, "對企業方案有興趣").await?;
    let contact = label_id(&mut tx, brand, "想聯絡業務").await?;
    let blog = label_id(&mut tx, brand, "部落格讀者").await?;
    let brand_segments = [
        SegmentSpec {
            name: "高意願名單",
            description: "最近兩週來過、看過定價，而且對企業方案有興趣或想聯絡業務的會員",
            kind: "member",
            rules: Rules {
                must: vec![Condition::Label(pricing), Condition::SeenWithin(14)],
                should: vec![Condition::Label(enterprise), Condition::Label(contact)],
                must_not: vec![],
            },
        },
        SegmentSpec {
            name: "搜尋引擎來的匿名訪客",
            description: "從 Google 進來、看了至少三頁，但還沒留下 email",
            kind: "anonymous",
            rules: Rules {
                must: vec![
                    Condition::Referrer("google".into()),
                    Condition::MinPageviews(3),
                ],
                ..Default::default()
            },
        },
        SegmentSpec {
            name: "沉睡會員",
            description: "超過兩週沒有再來的會員",
            kind: "member",
            rules: Rules {
                must_not: vec![Condition::SeenWithin(14)],
                ..Default::default()
            },
        },
        SegmentSpec {
            name: "本週新來的部落格讀者",
            description: "這七天第一次來，而且讀了兩篇以上的文章",
            kind: "all",
            rules: Rules {
                must: vec![Condition::FirstSeenWithin(7), Condition::Label(blog)],
                ..Default::default()
            },
        },
    ];
    for segment in brand_segments {
        add_segment(&mut tx, brand, owner, segment, now).await?;
    }

    let shop = seed_site(&mut tx, &mut rng, &SHOP, &mut emails, now).await?;
    let cart = label_id(&mut tx, shop, "加入購物車").await?;
    let ordered = label_id(&mut tx, shop, "已下單").await?;
    let beans = label_id(&mut tx, shop, "咖啡豆愛好者").await?;
    let shop_segments = [
        SegmentSpec {
            name: "放進購物車但沒結帳",
            description: "加過購物車、還沒下過單的人",
            kind: "all",
            rules: Rules {
                must: vec![Condition::Label(cart)],
                must_not: vec![Condition::Label(ordered)],
                ..Default::default()
            },
        },
        SegmentSpec {
            name: "咖啡豆回購名單",
            description: "買過東西、又常回來看咖啡豆的會員",
            kind: "member",
            rules: Rules {
                must: vec![Condition::Label(ordered), Condition::Label(beans)],
                ..Default::default()
            },
        },
    ];
    for segment in shop_segments {
        add_segment(&mut tx, shop, owner, segment, now).await?;
    }
    tx.commit().await?;

    let people: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM people")
        .fetch_one(&state.db)
        .await?;
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&state.db)
        .await?;
    Ok(Summary {
        users: accounts.len(),
        sites: 2,
        people,
        events,
    })
}
