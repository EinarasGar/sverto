import { useMutation, useQueryClient } from "@tanstack/react-query"

import type { CreatePropertyRequest, UpdateProperty } from "@/api"
import {
  IndividualTransactionsApiFactory,
  PropertiesApiFactory,
  UserAssetsApiFactory,
} from "@/api"
import { transactionDetailQueryOptions } from "@/features/transactions/api"
import { api } from "@/lib/api"
import type { AccountId, AssetId, PropertyId, UserId } from "@/lib/query"
import { mutationKeys, optimisticMutationOptions, queryKeys } from "@/lib/query"

import { fetchPropertyPurchase } from "./purchase"
import type { Valuation } from "./valuation-steps"
import { valuationEditSteps } from "./valuation-steps"

export interface CreatePropertyVariables {
  body: CreatePropertyRequest
}

export interface UpdatePropertyVariables {
  propertyId: PropertyId
  body: UpdateProperty
}

export interface PropertyLoanVariables {
  propertyId: PropertyId
  accountId: AccountId
}

export interface PropertyValuationPair {
  assetId: AssetId
  currencyAssetId: AssetId
}

export interface AddPropertyValuationVariables extends PropertyValuationPair {
  valuation: Valuation
}

export interface EditPropertyValuationVariables extends PropertyValuationPair {
  from: Valuation
  to: Valuation
}

export interface DeletePropertyValuationVariables extends PropertyValuationPair {
  date: number
}

export interface SetPropertyPurchaseDateVariables extends PropertyValuationPair {
  accountId: AccountId
  date: number
}

async function addValuation(
  userId: UserId,
  { assetId, currencyAssetId }: PropertyValuationPair,
  { date, value }: Valuation
) {
  await api(UserAssetsApiFactory).postCustomAssetRates(
    userId,
    assetId,
    currencyAssetId,
    { rates: [{ date, rate: value }] }
  )
}

// Stored valuation times keep sub-second precision while the API speaks whole seconds, so
// the one-second window is what reaches the stored row.
async function deleteValuation(
  userId: UserId,
  { assetId, currencyAssetId }: PropertyValuationPair,
  date: number
) {
  await api(UserAssetsApiFactory).deleteAssetPairRates(
    userId,
    assetId,
    currencyAssetId,
    date,
    date + 1
  )
}

async function editValuation(
  userId: UserId,
  pair: PropertyValuationPair,
  from: Valuation,
  to: Valuation
) {
  for (const step of valuationEditSteps(from, to)) {
    if (step.kind === "add") await addValuation(userId, pair, step.valuation)
    else await deleteValuation(userId, pair, step.date)
  }
}

function propertyInvalidations(userId: UserId) {
  const keys = queryKeys.user(userId)
  return [
    keys.properties.all(),
    keys.accounts.all(),
    keys.portfolio.all(),
    keys.assets.all(),
    queryKeys.reference.assets.all(),
  ]
}

export function useCreateProperty(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<PropertyId, CreatePropertyVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ body }) => {
        const response = await api(PropertiesApiFactory).createProperty(
          userId,
          body
        )
        return response.data.property_id
      },
      updates: [],
      invalidate: [
        ...propertyInvalidations(userId),
        queryKeys.user(userId).transactions.all(),
      ],
      meta: { errorContext: "The property could not be added" },
    })
  )
}

export function useUpdateProperty(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, UpdatePropertyVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ propertyId, body }) => {
        await api(PropertiesApiFactory).updateProperty(userId, propertyId, body)
      },
      updates: [],
      invalidate: propertyInvalidations(userId),
      meta: { errorContext: "The property could not be saved" },
    })
  )
}

export function useLinkPropertyLoan(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, PropertyLoanVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ propertyId, accountId }) => {
        await api(PropertiesApiFactory).linkPropertyLoan(
          accountId,
          userId,
          propertyId
        )
      },
      updates: [],
      invalidate: propertyInvalidations(userId),
      meta: { errorContext: "The loan could not be linked" },
    })
  )
}

export function useUnlinkPropertyLoan(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, PropertyLoanVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ propertyId, accountId }) => {
        await api(PropertiesApiFactory).unlinkPropertyLoan(
          accountId,
          userId,
          propertyId
        )
      },
      updates: [],
      invalidate: propertyInvalidations(userId),
      meta: { errorContext: "The loan could not be unlinked" },
    })
  )
}

export function useAddPropertyValuation(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, AddPropertyValuationVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ valuation, ...pair }) => {
        await addValuation(userId, pair, valuation)
      },
      updates: [],
      invalidate: propertyInvalidations(userId),
      meta: { errorContext: "The valuation could not be saved" },
    })
  )
}

export function useEditPropertyValuation(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, EditPropertyValuationVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ from, to, ...pair }) => {
        await editValuation(userId, pair, from, to)
      },
      updates: [],
      invalidate: propertyInvalidations(userId),
      meta: { errorContext: "The valuation could not be saved" },
    })
  )
}

export function useDeletePropertyValuation(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, DeletePropertyValuationVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ date, ...pair }) => {
        await deleteValuation(userId, pair, date)
      },
      updates: [],
      invalidate: propertyInvalidations(userId),
      meta: { errorContext: "The valuation could not be deleted" },
    })
  )
}

export function useSetPropertyPurchaseDate(userId: UserId) {
  const queryClient = useQueryClient()
  return useMutation(
    optimisticMutationOptions<void, SetPropertyPurchaseDateVariables>({
      queryClient,
      mutationKey: mutationKeys.user(userId).properties(),
      mutationFn: async ({ accountId, date, ...pair }) => {
        const purchase = await fetchPropertyPurchase(
          userId,
          accountId,
          pair.assetId
        )
        if (purchase === null) {
          throw new Error("The purchase transaction could not be found.")
        }
        const { transaction } = await queryClient.fetchQuery({
          ...transactionDetailQueryOptions({
            userId,
            transactionId: purchase.transactionId,
          }),
          staleTime: 0,
        })
        await api(
          IndividualTransactionsApiFactory
        ).updateAnExistingIndividualTransaction(
          userId,
          purchase.transactionId,
          { transaction: { ...transaction, date } }
        )

        const rates = await api(UserAssetsApiFactory).getUserAssetPairRates(
          userId,
          pair.assetId,
          pair.currencyAssetId,
          "all"
        )
        const valuation = rates.data.rates.find(
          (rate) => rate.date === transaction.date
        )
        if (valuation === undefined || transaction.date === date) return
        await editValuation(
          userId,
          pair,
          { date: transaction.date, value: valuation.rate },
          { date, value: valuation.rate }
        )
      },
      updates: [],
      invalidate: [
        ...propertyInvalidations(userId),
        queryKeys.user(userId).transactions.all(),
      ],
      meta: { errorContext: "The purchase date could not be saved" },
    })
  )
}
