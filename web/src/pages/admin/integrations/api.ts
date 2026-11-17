import type { components } from '../../../api/schema';

// Types and query keys shared by the integrations pages.

export type Provider = components['schemas']['ProviderDto'];
export type Model = components['schemas']['ModelDto'];
export type ProviderRequest = components['schemas']['ProviderRequest'];
export type Defaults = components['schemas']['DefaultsDto'];
export type RemovalImpact = components['schemas']['RemovalImpactDto'];
export type SearchOverview = components['schemas']['SearchOverviewDto'];
export type SearchKind = components['schemas']['SearchKindDto'];
export type Connector = components['schemas']['ConnectorDto'];
export type PolicyView = components['schemas']['PolicyViewDto'];
export type Policy = components['schemas']['PolicyDto'];
export type Limits = components['schemas']['LimitsDto'];

export const PROVIDERS_KEY = ['admin', 'providers'] as const;
export const DEFAULTS_KEY = ['admin', 'model-defaults'] as const;
export const SEARCH_KEY = ['admin', 'search'] as const;
export const GITHUB_KEY = ['admin', 'connectors', 'github'] as const;
export const POLICY_KEY = ['admin', 'sandbox-policy'] as const;

/** While a test nobody on this page follows is running, refresh the list now and then. */
export const ACTIVE_TEST_REFRESH_MS = 2000;
