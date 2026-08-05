import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RecentFolders } from "./RecentFolders";
import type { RecentFolderData } from "../../api/commandEnvelope";

const folders: RecentFolderData[] = [
  { path: "/music/project", name: "project", last_opened_ms: 300 },
  { path: "/music/album", name: "album", last_opened_ms: 200 },
];

describe("RecentFolders", () => {
  it("renders entries as reopen buttons", () => {
    render(
      <RecentFolders
        folders={folders}
        isLoading={false}
        error={null}
        onReopen={vi.fn()}
        onClear={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("region", { name: "Recent folders" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "project" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "album" })).toBeInTheDocument();
  });

  it("reopens a folder with its full path", () => {
    const onReopen = vi.fn();
    render(
      <RecentFolders
        folders={folders}
        isLoading={false}
        error={null}
        onReopen={onReopen}
        onClear={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "project" }));

    expect(onReopen).toHaveBeenCalledWith("/music/project");
  });

  it("exposes the full path only as a hover title", () => {
    render(
      <RecentFolders
        folders={folders}
        isLoading={false}
        error={null}
        onReopen={vi.fn()}
        onClear={vi.fn()}
      />,
    );

    const entry = screen.getByRole("button", { name: "project" });
    expect(entry).toHaveAttribute("title", "/music/project");
    expect(entry.textContent).toBe("project");
  });

  it("clears the history through the clear action", () => {
    const onClear = vi.fn();
    render(
      <RecentFolders
        folders={folders}
        isLoading={false}
        error={null}
        onReopen={vi.fn()}
        onClear={onClear}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Clear recent folders" }),
    );

    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it("hides the clear action and shows an empty state without entries", () => {
    render(
      <RecentFolders
        folders={[]}
        isLoading={false}
        error={null}
        onReopen={vi.fn()}
        onClear={vi.fn()}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "Clear recent folders" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("No recent folders yet.")).toBeInTheDocument();
  });

  it("shows a loading placeholder while fetching", () => {
    render(
      <RecentFolders
        folders={[]}
        isLoading={true}
        error={null}
        onReopen={vi.fn()}
        onClear={vi.fn()}
      />,
    );

    expect(screen.getByText("Loading recent folders…")).toBeInTheDocument();
  });

  it("announces backend errors without blocking the list", () => {
    render(
      <RecentFolders
        folders={folders}
        isLoading={false}
        error="Recent folders are unavailable."
        onReopen={vi.fn()}
        onClear={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Recent folders are unavailable.",
    );
    expect(screen.getByRole("button", { name: "project" })).toBeInTheDocument();
  });
});
