import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import ErrorPage from "@/app/error";

it("offers error recovery through retry and a docs link", () => {
  const retry = vi.fn();
  render(<ErrorPage retry={retry} />);
  expect(
    screen.getByRole("heading", { level: 1, name: "Something went wrong" })
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  expect(retry).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("link", { name: "Browse documentation" })
  ).toHaveAttribute("href", "/docs");
});
