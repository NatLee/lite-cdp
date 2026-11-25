import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { fakeApi } from "../test/fakeApi";
import { admin, label, meta, person, segment, site } from "../test/fixtures";
import { renderApp } from "../test/render";

const common = {
  "GET /meta": () => meta,
  "GET /auth/state": () => ({ setup_required: false, user: admin }),
  "GET /sites": () => [site],
  "GET /sites/7/labels": () => [label],
};

test("改條件會重算人數，存檔只送填完的條件", async () => {
  const calls = fakeApi({
    ...common,
    "POST /sites/7/segments/preview": (body) => {
      const { rules } = body as { rules: { must: unknown[] } };
      return rules.must.length ? { count: 4, total: 12, sample: [person(1)] } : { count: 12, total: 12, sample: [] };
    },
    "POST /sites/7/segments": () => segment,
    "GET /sites/7/segments/9": () => segment,
  });
  renderApp("/s/7/segments/new");

  expect(await screen.findByTestId("preview-count")).toHaveTextContent("12");

  await userEvent.type(screen.getByLabelText("名稱"), "看過定價的會員");
  await userEvent.click(screen.getByRole("radio", { name: "會員" }));
  const [must, , mustNot] = screen.getAllByRole("button", { name: /加條件/ });
  await userEvent.click(must);
  // 有標籤的時候，新條件預設是第一個標籤
  expect(screen.getByLabelText("「每一條都要符合」第 1 個條件的標籤")).toHaveValue("3");
  await waitFor(() => expect(screen.getByTestId("preview-count")).toHaveTextContent("4"));
  expect(screen.getByText("訪客 1")).toBeInTheDocument();

  // 選了「Email 含有」但還沒打字：不送
  await userEvent.click(mustNot);
  await userEvent.selectOptions(screen.getByLabelText("「一條都不能符合」第 1 個條件的欄位"), "email");
  await userEvent.click(screen.getByRole("button", { name: "建立分群" }));

  await waitFor(() => expect(calls.some((c) => c.path === "/sites/7/segments")).toBe(true));
  expect(calls.find((c) => c.path === "/sites/7/segments")?.body).toEqual({
    name: "看過定價的會員",
    description: "",
    kind: "member",
    rules: { must: [{ field: "label", value: 3 }], should: [], must_not: [] },
  });
});

test("打開存好的分群會帶出原本的條件", async () => {
  fakeApi({
    ...common,
    "GET /sites/7/segments/9": () => segment,
    "POST /sites/7/segments/preview": () => ({ count: 4, total: 12, sample: [] }),
  });
  renderApp("/s/7/segments/9");

  expect(await screen.findByLabelText("名稱")).toHaveValue("看過定價的會員");
  expect(screen.getByRole("radio", { name: "會員" })).toBeChecked();
  expect(screen.getByRole("link", { name: /看名單/ })).toHaveAttribute("href", "/s/7/people?segment_id=9");
});
