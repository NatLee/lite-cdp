import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { expect, test } from "vitest";
import { AppProvider, useApp } from "../context";
import { ToastProvider } from "../components/ui";
import { Settings } from "../pages/Settings";
import { fakeApi } from "../test/fakeApi";

test("a registrant can edit and save a 100-character display name without truncation", async () => {
  const user = { id: 2, username: "new.reader", display_name: "Reader", role: "member", email: "", hotel_id: null };
  const calls = fakeApi({
    "GET /meta": () => ({ version: "1.0.0" }),
    "GET /auth/state": () => ({ setup_required: false, user }),
    "GET /groups": () => [],
    "PATCH /auth/me": body => ({ ...user, ...(body as object) }),
  });
  function SessionSettings() { const { auth } = useApp(); return auth?.user ? <Settings /> : null; }
  render(<MemoryRouter><AppProvider><ToastProvider><Routes><Route element={<Outlet context={{ hotels: { main: null, competitors: [] }, reloadHotels: async () => {} }} />}><Route index element={<SessionSettings />} /></Route></Routes></ToastProvider></AppProvider></MemoryRouter>);
  const name = await screen.findByLabelText("顯示名稱", { exact: true });
  const longName = "A".repeat(100);
  await userEvent.clear(name);
  await userEvent.type(name, longName);
  expect(name).toHaveValue(longName);
  await userEvent.click(within(name.closest("form")!).getByRole("button", { name: "儲存" }));
  await waitFor(() => expect(calls.find(call => call.method === "PATCH" && call.path === "/auth/me")?.body).toMatchObject({ display_name: longName }));
  expect(name).toHaveValue(longName);
});
