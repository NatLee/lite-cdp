import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { expect, test } from "vitest";
import { App } from "../App";
import { RegisterForm } from "./RegisterForm";
import { AppProvider, useApp } from "../context";
import { ToastProvider } from "../components/ui";
import { fakeApi } from "../test/fakeApi";

function show(path = "/register") {
  render(<MemoryRouter initialEntries={[path]}><AppProvider><ToastProvider><App /></ToastProvider></AppProvider></MemoryRouter>);
}
const base = {
  "GET /meta": () => ({ version: "1.0.0" }),
  "GET /auth/state": () => ({ setup_required: false, user: null }),
  "GET /access/providers": () => ({ items: [] }),
};
test("closed registration has no credential or provider signup actions", async () => {
  fakeApi({ ...base, "GET /auth/registration-policy": () => ({ mode: "closed", password_enabled: false, providers: [], setup_required: false }) });
  show();
  expect(await screen.findByText("目前未開放註冊")).toBeInTheDocument();
  expect(screen.queryByLabelText("確認密碼")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "建立帳號" })).not.toBeInTheDocument();
});
test("confirmation mismatch prevents signup and approval result stays outside workspace", async () => {
  const calls = fakeApi({ ...base,
    "GET /auth/registration-policy": () => ({ mode: "approval", password_enabled: true, providers: ["google"], setup_required: false }),
    "POST /auth/register": () => ({ status: "pending", user_id: 2 }),
  });
  show();
  await userEvent.type(await screen.findByLabelText("帳號"), "new.reader");
  await userEvent.type(screen.getByLabelText("密碼"), "long enough password");
  await userEvent.type(screen.getByLabelText("確認密碼"), "different password");
  await userEvent.click(screen.getByRole("button", { name: "建立帳號" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("兩次輸入的密碼不一致");
  expect(calls.filter(call => call.path === "/auth/register")).toHaveLength(0);
  await userEvent.clear(screen.getByLabelText("確認密碼"));
  await userEvent.type(screen.getByLabelText("確認密碼"), "long enough password");
  await userEvent.click(screen.getByRole("button", { name: "建立帳號" }));
  expect(await screen.findByRole("heading", { name: "申請已送出，等待審核" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "登出" })).not.toBeInTheDocument();
  expect(calls.find(call => call.path === "/auth/register")?.body).toEqual({ username: "new.reader", password: "long enough password" });
});
test("provider callback pending notice survives anonymous route canonicalization", async () => {
  fakeApi(base);
  show("/login?registration=pending");
  expect(await screen.findByText("申請已送出，等待管理員審核。核准後才能登入工作區。")).toBeInTheDocument();
});
test("signup exposes only configured providers and preserves only safe OAuth continuation", async () => {
  fakeApi({ ...base, "GET /auth/registration-policy": () => ({ mode: "open", password_enabled: false, providers: ["github"], setup_required: false }) });
  show("/register?next=%2Foauth%2Fauthorize%3Fclient_id%3Dpublic");
  expect(await screen.findByRole("link", { name: "使用 GitHub 註冊" })).toHaveAttribute("href", "/oauth/login/github?intent=register&next=%2Foauth%2Fauthorize%3Fclient_id%3Dpublic");
  expect(screen.queryByRole("link", { name: "使用 Google 註冊" })).not.toBeInTheDocument();
  expect(screen.queryByLabelText("密碼")).not.toBeInTheDocument();
});
test("bootstrap takes priority over registration and never opens member signup", async () => {
  const calls = fakeApi({ ...base, "GET /auth/state": () => ({ setup_required: true, user: null }) });
  show();
  expect(await screen.findByRole("button", { name: "建立管理員" })).toBeInTheDocument();
  expect(calls.some(call => call.path === "/auth/registration-policy")).toBe(false);
});
test("open signup refreshes the real session before entering the workspace", async () => {
  let registered = false;
  fakeApi({ ...base,
    "GET /auth/registration-policy": () => ({ mode: "open", password_enabled: true, providers: [], setup_required: false }),
    "POST /auth/register": () => { registered = true; return { status: "active", user_id: 2 }; },
    "GET /auth/state": () => ({ setup_required: false, user: registered ? { id: 2, username: "new.reader", display_name: "Reader", role: "member" } : null }),
  });
  function SessionProbe() { const { auth } = useApp(); return <output>{auth?.user ? "session connected" : "session anonymous"}</output>; }
  render(<MemoryRouter><AppProvider><RegisterForm /><SessionProbe /></AppProvider></MemoryRouter>);
  await userEvent.type(await screen.findByLabelText("帳號"), "new.reader");
  await userEvent.type(screen.getByLabelText("密碼"), "long enough password");
  await userEvent.type(screen.getByLabelText("確認密碼"), "long enough password");
  await userEvent.click(screen.getByRole("button", { name: "建立帳號" }));
  expect(await screen.findByText("session connected")).toBeInTheDocument();
});
test("untrusted continuation never reaches provider signup URLs", async () => {
  fakeApi({ ...base, "GET /auth/registration-policy": () => ({ mode: "open", password_enabled: false, providers: ["google"], setup_required: false }) });
  show("/register?next=https%3A%2F%2Fevil.example%2F");
  expect(await screen.findByRole("link", { name: "使用 Google 註冊" })).toHaveAttribute("href", "/oauth/login/google?intent=register");
});
