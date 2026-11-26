import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { admin, meta } from "../test/fixtures";
import { renderApp } from "../test/render";

test("手機選單與桌面的外觀選擇保持同步", async () => {
  localStorage.setItem("cdp.theme", "light");
  fakeApi({
    "GET /meta": () => meta,
    "GET /auth/state": () => ({ setup_required: false, user: admin }),
    "GET /sites": () => [],
  });
  renderApp();
  await userEvent.click(
    await screen.findByRole("button", { name: "開啟選單" }),
  );
  const controls = screen.getAllByRole("radiogroup", { name: "外觀" });
  expect(controls).toHaveLength(2);
  await userEvent.click(
    within(controls[1]).getByRole("radio", { name: "深色" }),
  );
  expect(document.documentElement).toHaveClass("dark");
  expect(
    within(controls[0]).getByRole("radio", { name: "深色" }),
  ).toHaveAttribute("aria-checked", "true");
  await userEvent.click(screen.getByRole("button", { name: "關閉選單" }));
  await userEvent.click(screen.getByRole("radio", { name: "淺色" }));
  expect(document.documentElement).not.toHaveClass("dark");
  expect(localStorage.getItem("cdp.theme")).toBe("light");
});
