import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { admin, label, meta, site } from "../test/fixtures";
import { renderApp } from "../test/render";

const common = {
  "GET /meta": () => meta,
  "GET /auth/state": () => ({ setup_required: false, user: admin }),
  "GET /sites": () => [site],
  "GET /sites/7/labels": () => [label],
};

test("新增標籤：沒填內容的規則不會送出去", async () => {
  const calls = fakeApi({ ...common, "POST /sites/7/labels": () => ({ ...label, id: 4, name: "加入購物車" }) });
  renderApp("/s/7/labels");

  await userEvent.click(await screen.findByRole("button", { name: /新增標籤/ }));
  const create = screen.getByRole("button", { name: "建立" });
  await userEvent.type(screen.getByLabelText("名稱"), "加入購物車");
  // 還沒有任何規則有內容
  expect(create).toBeDisabled();

  await userEvent.selectOptions(screen.getByLabelText("規則 1 的種類"), "event");
  await userEvent.type(screen.getByLabelText("規則 1 的內容"), "add_to_cart");
  await userEvent.click(screen.getByRole("button", { name: /再加一條/ }));
  await userEvent.click(screen.getByRole("radio", { name: "綠" }));
  await userEvent.click(create);

  await waitFor(() => expect(calls.some((c) => c.method === "POST")).toBe(true));
  expect(calls.find((c) => c.method === "POST")?.body).toEqual({
    name: "加入購物車",
    color: "green",
    description: "",
    min_hits: 1,
    rules: [{ kind: "event", value: "add_to_cart" }],
  });
});

test("從頁面帶路徑過來會打開填好的視窗", async () => {
  fakeApi(common);
  renderApp("/s/7/labels?path=%2Fblog");

  expect(await screen.findByLabelText("規則 1 的內容")).toHaveValue("/blog");
});

test("刪不掉（還有分群在用）時顯示後端的訊息", async () => {
  fakeApi(common);
  renderApp("/s/7/labels");

  await userEvent.click(await screen.findByRole("button", { name: "刪除 看過定價" }));
  await userEvent.click(screen.getByRole("button", { name: "刪除" }));

  expect(await screen.findByText(/no fake for DELETE/)).toBeInTheDocument();
});
