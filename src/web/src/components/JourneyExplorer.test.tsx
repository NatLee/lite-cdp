import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { expect, test } from "vitest";
import { JourneyExplorer, JourneyMap, sessionGroups } from "./JourneyExplorer";
import type { TimelineEvent } from "../types";
import { fakeApi } from "../test/fakeApi";
import { person } from "../test/fixtures";

const event = (
  id: number,
  session_id: string,
  overrides: Partial<TimelineEvent> = {},
): TimelineEvent => ({
  id,
  session_id,
  type: "pageview",
  name: "",
  title: `Page ${id}`,
  path: `/page-${id}`,
  url: "",
  referrer_host: "",
  props: {},
  created_at: `2026-03-0${id}T08:00:00Z`,
  ...overrides,
});

test("sessions preserve recorded boundaries, chronological order, and separate missing IDs", () => {
  const result = sessionGroups([
    event(3, "a"),
    event(1, "a"),
    event(2, "b"),
    event(4, ""),
    event(5, ""),
  ]);
  expect(result.map((g) => g.items.map((e) => e.id))).toEqual([
    [5],
    [4],
    [1, 3],
    [2],
  ]);
});

test("session selector hides other routes and keyboard focus exposes the exact event properties", async () => {
  render(
    <JourneyMap
      events={[
        event(3, "a", {
          type: "custom",
          name: "request_demo",
          props: { source: "pricing" },
        }),
        event(1, "a"),
        event(2, "b"),
      ]}
    />,
  );
  expect(
    screen.queryByRole("button", { name: /Page 2/ }),
  ).not.toBeInTheDocument();
  const target = screen.getByRole("button", { name: /request_demo/ });
  target.focus();
  await userEvent.keyboard("{Enter}");
  expect(target).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByText("source: pricing")).toBeInTheDocument();
  expect(screen.getByText(/節點等距，非時間比例/)).toBeInTheDocument();
  await userEvent.selectOptions(screen.getByLabelText("選擇來訪"), "session:b");
  expect(
    screen.queryByRole("button", { name: /request_demo/ }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Page 2/ })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(screen.queryByText("source: pricing")).not.toBeInTheDocument();
});

test("selecting a visitor fetches only that visitor's observed events and updates the detail link", async () => {
  const calls = fakeApi({
    "GET /sites/7/people": () => ({
      items: [person(1), person(2)],
      total: 2,
      page: 1,
      per_page: 6,
    }),
    "GET /sites/7/people/1/events": () => ({
      items: [event(1, "a")],
      next_before: null,
    }),
    "GET /sites/7/people/2/events": () => ({
      items: [event(2, "b")],
      next_before: null,
    }),
  });
  render(
    <MemoryRouter>
      <JourneyExplorer siteId={7} />
    </MemoryRouter>,
  );
  await screen.findByRole("button", { name: /Page 1/ });
  await userEvent.click(screen.getByRole("button", { name: /訪客 2/ }));
  await screen.findByRole("button", { name: /Page 2/ });
  expect(
    screen.queryByRole("button", { name: /Page 1/ }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: /開啟個人旅程/ })).toHaveAttribute(
    "href",
    "/s/7/people/2",
  );
  expect(calls.filter((c) => c.path.endsWith("/events"))).toHaveLength(2);
  expect(
    within(screen.getByLabelText("選擇來訪")).getAllByRole("option"),
  ).toHaveLength(1);
});

test("an empty journey explains the absence of observed events", () => {
  render(<JourneyMap events={[]} />);
  expect(screen.getByText("這位訪客還沒有事件紀錄。")).toBeInTheDocument();
});

test("arrow keys move event focus within the selected session and retain keyboard focus", async () => {
  render(<JourneyMap events={[event(1, "a"), event(3, "a", { type: "custom", name: "request_demo", props: { plan: "team" } }), event(2, "b")]} />);
  const first = screen.getByRole("button", { name: /Page 1/ });
  const last = screen.getByRole("button", { name: /request_demo/ });
  first.focus();
  await userEvent.keyboard("{ArrowRight}");
  expect(last).toHaveFocus();
  expect(last).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByText("plan: team")).toBeInTheDocument();
  expect(screen.getByLabelText("選擇來訪")).toHaveValue("session:a");
  await userEvent.keyboard("{Home}");
  expect(first).toHaveFocus();
  expect(first).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryByText("plan: team")).not.toBeInTheDocument();
});
