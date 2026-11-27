import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router";
import { expect, test } from "vitest";
import { App } from "../App";
import { AppProvider } from "../context";
import { ToastProvider } from "../components/ui";
import { ApiError, api } from "../api";
import { fakeApi } from "../test/fakeApi";
import { vi } from "vitest";

function Location() { const l = useLocation(); return <output data-testid="location">{l.pathname}{l.search}</output>; }
function show(path = "/") { render(<MemoryRouter initialEntries={[path]}><AppProvider><ToastProvider><App/><Location/></ToastProvider></AppProvider></MemoryRouter>); }
function empty() { return fakeApi({ "GET /auth/state": () => ({ setup_required: true, user: null }) }); }
async function fill(confirm = "correct horse") {
  await userEvent.type(await screen.findByLabelText("帳號"), "nat");
  await userEvent.type(screen.getByLabelText("密碼"), "correct horse");
  await userEvent.type(screen.getByLabelText("確認密碼"), confirm);
}

test.each(["/", "/login", "/settings"])("empty entry %s opens dedicated first-login", async path => {
  empty(); show(path);
  expect(await screen.findByRole("heading", {name: "建立第一位管理員"})).toBeInTheDocument();
  expect(screen.getByTestId("location")).toHaveTextContent(/^\/first-login$/);
});

test("mismatched confirmation creates no request", async () => {
  const calls = empty(); show(); await fill("different password");
  await userEvent.click(screen.getByRole("button", {name: "建立管理員"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("兩次輸入的密碼不一致");
  expect(calls.filter(c=>c.path === "/auth/setup")).toHaveLength(0);
});

test("initialized installation closes first-login and strips unsafe next", async () => {
  fakeApi({"GET /auth/state": () => ({setup_required: false, user: null})});
  show("/first-login?next=https://evil.invalid");
  expect(await screen.findByRole("button", {name: "登入"})).toBeInTheDocument();
  expect(screen.getByTestId("location")).toHaveTextContent(/^\/login$/);
  expect(screen.queryByLabelText("確認密碼")).not.toBeInTheDocument();
});

test("empty entry preserves validated OAuth continuation", async () => {
  empty(); show("/login?next=%2Foauth%2Fauthorize%3Fclient_id%3Dexample");
  await screen.findByLabelText("確認密碼");
  expect(screen.getByTestId("location")).toHaveTextContent("/first-login?next=%2Foauth%2Fauthorize%3Fclient_id%3Dexample");
});

test("409 refreshes state and closes stale setup", async () => {
  let initialized = false;
  fakeApi({"GET /auth/state": () => ({setup_required: !initialized, user: null}), "POST /auth/setup": () => {initialized=true; throw new ApiError(409, "already initialized", null);}});
  show(); await fill(); await userEvent.click(screen.getByRole("button", {name: "建立管理員"}));
  expect(await screen.findByRole("button", {name: "登入"})).toBeInTheDocument();
  expect(screen.getByTestId("location")).toHaveTextContent(/^\/login$/);
});

test("failed setup shows server error and allows retry", async () => {
  fakeApi({"GET /auth/state": () => ({setup_required:true,user:null}), "POST /auth/setup": () => {throw new ApiError(500,"暫時無法建立",null);}});
  show(); await fill(); await userEvent.click(screen.getByRole("button", {name:"建立管理員"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("暫時無法建立");
  expect(screen.getByRole("button", {name:"建立管理員"})).toBeEnabled();
});

test("pending setup rejects duplicate form submissions", async () => {
  empty();
  let finish!: () => void;
  const pending = new Promise<never>((_, reject) => {finish = () => reject(new Error("retry later"));});
  const setup = vi.spyOn(api,"setup").mockReturnValue(pending);
  show(); await fill();
  const button=screen.getByRole("button", {name:"建立管理員"});
  fireEvent.submit(button.closest("form")!); fireEvent.submit(button.closest("form")!);
  await waitFor(()=>expect(button).toBeDisabled());
  expect(setup).toHaveBeenCalledTimes(1);
  finish(); await screen.findByRole("alert"); setup.mockRestore();
});


test("conflict closes setup even if refreshing state is unavailable", async () => {
  let initialized=false;
  fakeApi({"GET /auth/state": () => {if(initialized) throw new Error("offline"); return {setup_required:true,user:null};}, "POST /auth/setup": () => {initialized=true; throw new ApiError(409,"already initialized",null);}});
  show(); await fill(); await userEvent.click(screen.getByRole("button", {name:"建立管理員"}));
  expect(await screen.findByRole("button", {name:"登入"})).toBeInTheDocument();
  expect(screen.queryByLabelText("確認密碼")).not.toBeInTheDocument();
});


test("login retains recognized external-provider failure feedback", async () => {
  fakeApi({"GET /auth/state": () => ({setup_required:false,user:null})});
  show("/login?access_error=access_denied");
  await screen.findByRole("button", {name:"登入"});
  expect(screen.getByTestId("location")).toHaveTextContent("/login?access_error=access_denied");
});
