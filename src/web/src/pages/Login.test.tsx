import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { admin, meta } from "../test/fixtures";
import { renderApp } from "../test/render";

const common = {
  "GET /meta": () => meta,
  "GET /sites": () => [],
};

test("第一次使用要先建立管理員", async () => {
  const calls = fakeApi({
    ...common,
    "GET /auth/state": () => ({ setup_required: true, user: null }),
    "POST /auth/setup": () => admin,
  });
  renderApp();

  await userEvent.type(await screen.findByLabelText("帳號"), "nat");
  await userEvent.type(screen.getByLabelText("密碼"), "correct horse");
  await userEvent.type(screen.getByLabelText("確認密碼"), "correct horse");
  await userEvent.click(screen.getByRole("button", { name: "建立管理員" }));

  expect(await screen.findByRole("button", { name: "登出" })).toBeInTheDocument();
  // 顯示名稱沒填就不送
  expect(calls.find((c) => c.path === "/auth/setup")?.body).toEqual({ username: "nat", password: "correct horse" });
});

test("密碼錯了會顯示後端的訊息，並清掉密碼", async () => {
  fakeApi({
    ...common,
    "GET /auth/state": () => ({ setup_required: false, user: null }),
  });
  renderApp();

  await userEvent.type(await screen.findByLabelText("帳號"), "nat");
  await userEvent.type(screen.getByLabelText("密碼"), "wrong password");
  await userEvent.click(screen.getByRole("button", { name: "登入" }));

  expect(await screen.findByRole("alert")).toHaveTextContent("no fake for POST /auth/login");
  expect(screen.getByLabelText("密碼")).toHaveValue("");
});
