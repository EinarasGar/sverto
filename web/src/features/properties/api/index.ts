export type { PropertiesView, Property } from "./properties"
export {
  buildPropertiesView,
  propertiesQueryOptions,
  usePropertiesSuspense,
} from "./properties"

export type {
  AddPropertyValuationVariables,
  CreatePropertyVariables,
  DeletePropertyValuationVariables,
  EditPropertyValuationVariables,
  PropertyLoanVariables,
  SetPropertyPurchaseDateVariables,
  UpdatePropertyVariables,
} from "./mutations"
export {
  useAddPropertyValuation,
  useCreateProperty,
  useDeletePropertyValuation,
  useEditPropertyValuation,
  useLinkPropertyLoan,
  useSetPropertyPurchaseDate,
  useUnlinkPropertyLoan,
  useUpdateProperty,
} from "./mutations"

export { propertyPurchaseQueryOptions } from "./purchase"

export type { Valuation } from "./valuation-steps"
