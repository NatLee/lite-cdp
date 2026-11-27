import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { accounts, admin, emptyOverview, member, meta, site } from "../test/fixtures";
import { renderApp } from "../test/render";

const common = {
  "GET /meta": () => meta,
  "GET /sites": () => [site],
};

test("追蹤程式碼用對外的網址和這個網站的金鑰；成員看不到設定", async () => {
  fakeApi({ ...common, "GET /auth/state": () => ({ setup_required: false, user: member }) });
  renderApp("/s/7/install");

  expect(await screen.findByLabelText("追蹤程式碼")).toHaveTextContent(
    '<script async src="https://cdp.example.com/t.js" data-key="abcd1234abcd1234"></script>',
  );
  expect(screen.getByText(/已收到資料/)).toBeInTheDocument();
  expect(screen.queryByText("網站設定")).not.toBeInTheDocument();
});

test("管理員可以改允許的來源和成員", async () => {
  const calls = fakeApi({
    ...common,
    "GET /auth/state": () => ({ setup_required: false, user: admin }),
    "GET /users": () => accounts,
    "PATCH /sites/7": () => site,
  });
  renderApp("/s/7/install");

  await userEvent.type(await screen.findByLabelText("允許的來源"), "\nhttps://shop.example.com");
  await userEvent.click(await screen.findByRole("checkbox", { name: /Amy/ }));
  await userEvent.click(screen.getByRole("button", { name: "儲存" }));

  await waitFor(() => expect(calls.some((c) => c.method === "PATCH")).toBe(true));
  expect(calls.find((c) => c.method === "PATCH")?.body).toEqual({
    name: "官方網站",
    url: "https://www.example.com",
    allowed_origins: ["https://www.example.com", "https://shop.example.com"],
    active: true,
    member_ids: [2],
  });
});

test("首頁會進第一個網站的總覽；還沒有資料時帶去看追蹤程式碼", async () => {
  fakeApi({
    ...common,
    "GET /auth/state": () => ({ setup_required: false, user: admin }),
    "GET /sites/7/overview": () => emptyOverview,
  });
  renderApp("/");

  expect(await screen.findByText("還沒有收到資料")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /看追蹤程式碼/ })).toHaveAttribute("href", "/s/7/install");
});

test("網址裡的網站看不到時說找不到", async () => {
  fakeApi({ ...common, "GET /auth/state": () => ({ setup_required: false, user: member }) });
  renderApp("/s/99/people");

  expect(await screen.findByText("找不到這個網站")).toBeInTheDocument();
});

test("框架教學提供 Next 根版型且不重複送 router pageview", async () => {
  fakeApi({ ...common, "GET /auth/state": () => ({ setup_required: false, user: member }) });
  renderApp("/s/7/install");
  await userEvent.click(await screen.findByRole("tab", { name: "Next.js" }));
  expect(screen.getByLabelText("追蹤程式碼")).toHaveTextContent('strategy="afterInteractive"');
  expect(screen.getByLabelText("追蹤程式碼")).toHaveTextContent('data-key="abcd1234abcd1234"');
  expect(screen.getByText(/不要再加 router/)).toBeInTheDocument();
});

test("模擬器只在明確相同會員 ID 登入後合併裝置", async () => {
  const calls = fakeApi({ ...common, "GET /auth/state": () => ({ setup_required: false, user: member }) });
  renderApp("/s/7/install");
  await userEvent.click(await screen.findByRole("button", { name: "登入同一會員" }));
  await userEvent.click(screen.getByRole("button", { name: "另一台裝置" }));
  expect(screen.getByLabelText("模擬人物數")).toHaveTextContent("2");
  await userEvent.click(screen.getByRole("button", { name: "登入同一會員" }));
  expect(screen.getByLabelText("模擬人物數")).toHaveTextContent("1");
  expect(calls.filter(c => c.method === "POST")).toHaveLength(0);
});

test("站點採集政策明確儲存並生成可見設定", async () => {
  const calls = fakeApi({ ...common, "GET /auth/state": () => ({ setup_required: false, user: admin }), "GET /users": () => accounts, "PATCH /sites/7": () => site });
  renderApp("/s/7/install");
  await userEvent.click(await screen.findByRole("button", { name: "使用最少採集設定" }));
  expect(screen.getByRole("checkbox", { name: "要求訪客同意" })).toBeChecked();
  await userEvent.click(screen.getByRole("button", { name: "儲存採集設定" }));
  await waitFor(() => expect(calls.some(c => c.method === "PATCH")).toBe(true));
  expect(calls.find(c => c.method === "PATCH")?.body).toMatchObject({ collection_policy: {consent_required:true,auto_click:false,query_mode:"strip"} });
});

test("模擬器拒絕期間不建立編號，再次同意從新匿名開始", async () => {
  fakeApi({ ...common, "GET /auth/state": () => ({ setup_required: false, user: member }) });
  renderApp("/s/7/install");
  await userEvent.click(await screen.findByRole("button", { name: "登入同一會員" }));
  await userEvent.click(screen.getByRole("button", { name: "拒絕同意" }));
  await userEvent.click(screen.getByRole("button", { name: "另一台裝置" }));
  expect(screen.getByLabelText("模擬人物數")).toHaveTextContent("1");
  expect(screen.getByLabelText("模擬 payload")).toHaveTextContent("沒有 ID");
  await userEvent.click(screen.getByRole("button", { name: "同意採集" }));
  expect(screen.getByLabelText("模擬 payload")).not.toHaveTextContent('"identify"');
  expect(screen.getByLabelText("模擬 payload")).toHaveTextContent("demo-consent");
});
