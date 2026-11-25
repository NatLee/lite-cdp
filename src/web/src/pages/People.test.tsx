import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { admin, detail, label, member, meta, person, segment, site } from "../test/fixtures";
import { renderApp } from "../test/render";

const common = {
  "GET /meta": () => meta,
  "GET /auth/state": () => ({ setup_required: false, user: admin }),
  "GET /sites": () => [site],
  "GET /sites/7/labels": () => [label],
  "GET /sites/7/segments": () => [segment],
};

test("篩選條件會送給後端，匯出的連結也帶著同樣的條件", async () => {
  const queries: string[] = [];
  fakeApi({
    ...common,
    "GET /sites/7/people": (_body, url) => {
      queries.push(url.search);
      return { items: [person(1, { labels: [label] }), person(2, { email: "", name: "" })], total: 2, page: 1, per_page: 25 };
    },
  });
  renderApp("/s/7/people");

  expect(await screen.findByText("訪客 1")).toBeInTheDocument();
  // 匿名訪客沒有名字，用編號
  expect(screen.getByText("匿名訪客 #2")).toBeInTheDocument();
  expect(screen.getByText("共 2 人")).toBeInTheDocument();

  await userEvent.click(screen.getByRole("tab", { name: "會員" }));
  await userEvent.selectOptions(await screen.findByLabelText("標籤"), "看過定價");
  await waitFor(() => expect(queries.at(-1)).toContain("label_id=3"));
  expect(queries.at(-1)).toContain("kind=member");

  const href = screen.getByRole("link", { name: /匯出 CSV/ }).getAttribute("href");
  expect(href).toBe("/api/sites/7/people/export?kind=member&label_id=3");

  await userEvent.click(screen.getByRole("button", { name: /清除/ }));
  await waitFor(() => expect(queries.at(-1)).toBe("?page=1&per_page=25"));
});

test("個人頁面有資料、標籤和時間軸；成員不能刪除", async () => {
  const calls = fakeApi({
    ...common,
    "GET /auth/state": () => ({ setup_required: false, user: member }),
    "GET /sites/7/people/5": () => detail,
    "GET /sites/7/people/5/events": (_body, url) =>
      url.searchParams.get("before")
        ? { items: [], next_before: null }
        : {
            items: [
              { id: 12, type: "custom", name: "add_to_cart", url: "", path: "/pricing", title: "", referrer_host: "", props: { sku: "A-1" }, session_id: "s", created_at: "2026-03-01T08:05:00Z" },
              { id: 11, type: "pageview", name: "", url: "https://www.example.com/pricing", path: "/pricing", title: "定價方案", referrer_host: "www.google.com", props: {}, session_id: "s", created_at: "2026-03-01T08:00:00Z" },
            ],
            next_before: 11,
          },
  });
  renderApp("/s/7/people/5");

  expect(await screen.findByRole("heading", { name: /王小明/ })).toBeInTheDocument();
  expect(screen.getByText("plan")).toBeInTheDocument();
  expect(await screen.findByText("add_to_cart")).toBeInTheDocument();
  expect(screen.getByText("sku: A-1")).toBeInTheDocument();
  expect(screen.getByText("/pricing・從 www.google.com 來")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /刪除資料/ })).not.toBeInTheDocument();

  await userEvent.click(screen.getByRole("button", { name: "更早的" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "更早的" })).not.toBeInTheDocument());
  expect(calls.filter((c) => c.path === "/sites/7/people/5/events")).toHaveLength(2);
  // 側邊欄的網站選單選著這個網站
  expect(within(screen.getByLabelText("網站")).getByRole("option", { name: "官方網站" })).toBeInTheDocument();
});
