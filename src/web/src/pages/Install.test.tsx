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
