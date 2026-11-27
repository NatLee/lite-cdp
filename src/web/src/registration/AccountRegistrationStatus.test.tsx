import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { AccountRegistrationStatus } from "./AccountRegistrationStatus";
test("pending and rejected users retain their review state independently of enablement", () => {
 const { rerender } = render(<AccountRegistrationStatus status="pending" />);
 expect(screen.getByText("待審核・尚未開通")).toBeInTheDocument();
 rerender(<AccountRegistrationStatus status="rejected" />);
 expect(screen.getByText("申請已拒絕")).toBeInTheDocument();
 rerender(<AccountRegistrationStatus status="active" />);
 expect(screen.queryByText("待審核・尚未開通")).not.toBeInTheDocument();
});
