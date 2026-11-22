import { Link } from "@tanstack/react-router"

import { EmptyState } from "@/components/states/empty-state"

export function PropertyNotFound() {
  return (
    <EmptyState
      size="page"
      headline="This property is not on your profile"
      body="Its account may have been deactivated, or the link is from another profile."
      footnote={
        <Link to="/accounts" className="font-semibold text-brand">
          Back to accounts →
        </Link>
      }
    />
  )
}
