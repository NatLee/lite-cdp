import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { expect, test } from "vitest";
import { EngagementAtlas, TrafficExplorer, share } from "./EngagementAtlas";
import { admin, emptyOverview, meta, person, site } from "../test/fixtures";
import { fakeApi } from "../test/fakeApi";
import { renderApp } from "../test/render";

test("cohorts use the full audience denominator and link to the matching identity and activity", () => {
  expect(share(1, 4)).toBe("25.0%");
  expect(share(0, 0)).toBe("0.0%");
  render(
    <MemoryRouter>
      <EngagementAtlas
        base="/s/7"
        data={{
          ...emptyOverview,
          totals: { people: 4, identified: 1, anonymous: 3 },
          engagement: [{ activity: "recent", member: 1, anonymous: 3 }],
        }}
      />
    </MemoryRouter>,
  );
  expect(
    screen.getByRole("link", { name: "近期活躍的會員，1 人，查看名單" }),
  ).toHaveAttribute("href", "/s/7/people?kind=member&activity=recent");
  expect(
    screen.getByRole("link", { name: "久未回訪的匿名訪客，0 人，查看名單" }),
  ).toHaveTextContent("0.0%");
  expect(screen.getByText("占全部 25.0%")).toBeInTheDocument();
});
test("traffic explorer changes metric and exposes exact selected daily values via keyboard", async () => {
  render(
    <TrafficExplorer
      daily={[
        { date: "2026-03-01", pageviews: 20, visitors: 8 },
        { date: "2026-03-02", pageviews: 7, visitors: 3 },
      ]}
    />,
  );
  expect(screen.getByText(/最高單日/)).toHaveTextContent("2026-03-01 · 20 頁");
  await userEvent.click(screen.getByRole("button", { name: "每日訪客" }));
  expect(screen.getByText(/最高單日/)).toHaveTextContent("2026-03-01 · 8 人");
  const day = screen.getByRole("button", {
    name: "2026-03-01，20 頁，8 位訪客",
  });
  day.focus();
  await waitFor(() => expect(day).toHaveAttribute("aria-pressed", "true"));
  expect(screen.getByText("瀏覽 20 頁")).toBeInTheDocument();
  expect(screen.getByText(/跨日人數不可相加/)).toBeInTheDocument();
});
test("cohort filters reach API and CSV and can be cleared", async () => {
  const queries: URLSearchParams[] = [];
  fakeApi({
    "GET /meta": () => meta,
    "GET /auth/state": () => ({ setup_required: false, user: admin }),
    "GET /sites": () => [site],
    "GET /sites/7/labels": () => [],
    "GET /sites/7/segments": () => [],
    "GET /sites/7/people": (_body, url) => {
      queries.push(url.searchParams);
      return { items: [person(1)], total: 1, page: 1, per_page: 25 };
    },
  });
  renderApp("/s/7/people?kind=member&activity=cooling");
  await screen.findByText("訪客 1");
  expect(queries.at(-1)?.get("activity")).toBe("cooling");
  expect(queries.at(-1)?.get("kind")).toBe("member");
  expect(screen.getByLabelText("最近活躍")).toHaveValue("cooling");
  expect(screen.getByRole("link", { name: /匯出 CSV/ })).toHaveAttribute(
    "href",
    "/api/sites/7/people/export?activity=cooling&kind=member",
  );
  await userEvent.click(screen.getByRole("button", { name: /清除/ }));
  await waitFor(() => expect(queries.at(-1)?.has("activity")).toBe(false));
});
