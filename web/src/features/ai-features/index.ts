export {
  AI_FEATURES_FOOTER,
  AI_FEATURES_INTRO,
  AI_FEATURES_SETTING_CONSEQUENCE,
  AI_FEATURES_TITLE,
  KEEP_AI_FEATURES_OFF,
  TURN_ON_AI_FEATURES,
} from "./copy"

export {
  setCachedAiFeaturesEnabled,
  useAiFeaturesEnabled,
  useSetAiFeatures,
} from "./api"

export { AiFeaturesSharedData, PrivacyPolicyLink } from "./disclosure"

export type { RunWithAiConsent } from "./use-ai-features-gate"
export { useAiFeaturesGate } from "./use-ai-features-gate"
