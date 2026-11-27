import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { RegistrationAdmin } from "./RegistrationAdmin";
import { fakeApi } from "../test/fakeApi";
const application = { id: 10, user_id: 2, username: "new.reader", display_name: "New Reader", provider: "password", status: "pending", created_at: "2026-11-27T08:00:00Z", reviewed_at: null, reviewed_by: null, rejection_reason: null };
const policy = { mode: "closed", password_enabled: false, providers: [], setup_required: false };
const routes = {
  "GET /auth/registration-policy": () => policy,
  "GET /auth/registrations": () => ({ items: [application], next_cursor: null }),
  "GET /auth/registration-events": () => ({ items: [], next_cursor: null }),
};
test("admin saves registration policy and requires a rejection reason before review", async () => {
  let reviewed = false;
  const calls = fakeApi({ ...routes,
    "PATCH /auth/registration-policy": body => ({ ...policy, ...(body as object) }),
    "GET /auth/registrations": () => ({ items: reviewed ? [] : [application], next_cursor: null }),
    "POST /auth/registrations/10/review": () => { reviewed = true; return { ...application, status: "rejected" }; },
  });
  render(<RegistrationAdmin />);
  await waitFor(() => expect(screen.getByLabelText("註冊方式")).toBeEnabled());
  await userEvent.selectOptions(screen.getByLabelText("註冊方式"), "approval");
  await userEvent.click(screen.getByRole("button", { name: "儲存註冊方式" }));
  expect(await screen.findByText("註冊方式已更新。")).toBeInTheDocument();
  expect(calls.find(call => call.method === "PATCH")?.body).toEqual({ mode: "approval" });
  expect(screen.getByRole("button", { name: "拒絕 new.reader" })).toBeDisabled();
  await userEvent.type(screen.getByLabelText("new.reader 的拒絕原因"), "請向團隊確認申請資格");
  await userEvent.click(screen.getByRole("button", { name: "拒絕 new.reader" }));
  await waitFor(() => expect(screen.queryByText("New Reader")).not.toBeInTheDocument());
  expect(calls.find(call => call.method === "POST")?.body).toEqual({ decision: "reject", reason: "請向團隊確認申請資格" });
});
test("review queue and audit cursor pages append and filtering resets the queue", async () => {
  fakeApi({ ...routes,
    "GET /auth/registrations": (_body, url) => ({ items: url.searchParams.get("status") === "rejected" ? [] : url.searchParams.has("before") ? [{ ...application, id: 9, username: "older.reader", display_name: "Older Reader" }] : [application], next_cursor: url.searchParams.has("before") ? null : 10 }),
    "GET /auth/registration-events": (_body, url) => ({ items: [{ id: url.searchParams.has("before") ? 1 : 2, actor_id: 1, actor_username: "admin", action: "policy_changed", registration_id: null, mode: url.searchParams.has("before") ? "closed" : "approval", reason: null, created_at: "2026-11-27T08:00:00Z" }], next_cursor: url.searchParams.has("before") ? null : 2 }),
  });
  render(<RegistrationAdmin />);
  await userEvent.click(await screen.findByRole("button", { name: "載入更多申請" }));
  expect(await screen.findByText("Older Reader")).toBeInTheDocument();
  expect(screen.getByText("New Reader")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "載入更多註冊紀錄" }));
  expect(await screen.findByText(/更改註冊方式：關閉/)).toBeInTheDocument();
  await userEvent.selectOptions(screen.getByLabelText("申請狀態"), "rejected");
  await waitFor(() => expect(screen.queryByText("New Reader")).not.toBeInTheDocument());
  expect(await screen.findByText("沒有符合條件的申請。")).toBeInTheDocument();
});
test("stale review refreshes the queue and reports the conflict", async () => {
  let available = true;
  fakeApi({ ...routes, "GET /auth/registrations": () => ({ items: available ? [application] : [], next_cursor: null }) });
  const fetch = globalThis.fetch;
  globalThis.fetch = async (input, init) => {
    if (String(input).endsWith("registrations/10/review")) { available = false; return new Response(JSON.stringify({ error: "registration already reviewed" }), { status: 409 }); }
    return fetch(input, init);
  };
  render(<RegistrationAdmin />);
  await userEvent.click(await screen.findByRole("button", { name: "核准 new.reader" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("已由其他管理員處理");
  expect(screen.queryByText("New Reader")).not.toBeInTheDocument();
});
