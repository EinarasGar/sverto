import { Car, House, type LucideIcon } from "lucide-react"

import type { PropertyKind } from "@/api"

export const PROPERTY_KIND_LABELS: Record<PropertyKind, string> = {
  real_estate: "Real estate",
  vehicle: "Vehicle",
}

export const PROPERTY_KIND_ICONS: Record<PropertyKind, LucideIcon> = {
  real_estate: House,
  vehicle: Car,
}

export const ADD_PROPERTY = "Add property"
