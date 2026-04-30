import type { LinearIssue, Slot } from "./types.js";

/**
 * Minimal Linear API surface the orchestrator depends on. Wrapping the SDK
 * behind an interface keeps the webhook handler trivially mockable in tests
 * without booting `@linear/sdk` or hitting the network.
 */
export interface LinearClient {
  /**
   * Returns the next ticket for `slot` that:
   *  - is in `unstarted` state
   *  - is labelled "unblocked"
   *  - has the highest priority (Linear treats 0 = none, 1 = urgent, 4 = low)
   *
   * Highest priority = lowest non-zero number; ties broken by created date asc.
   * Returns null if the slot's queue is empty.
   */
  nextTicketForSlot(slot: Slot): Promise<LinearIssue | null>;

  /** Move an issue into the configured "In Progress" workflow state. */
  markInProgress(issueId: string): Promise<void>;
}

interface LinearGraphQLEdge<T> {
  node: T;
}

interface LinearIssuesResponse {
  data?: {
    issues?: {
      nodes?: LinearIssue[];
      edges?: LinearGraphQLEdge<LinearIssue>[];
    };
  };
  errors?: { message: string }[];
}

const LINEAR_GQL_ENDPOINT = "https://api.linear.app/graphql";

/**
 * Production implementation backed by Linear's GraphQL API. We use raw fetch
 * (rather than `@linear/sdk`'s typed client) to keep the dependency surface
 * minimal in serverless cold-starts and to make our own queries explicit.
 */
export class LinearGraphQLClient implements LinearClient {
  constructor(
    private readonly apiKey: string,
    private readonly inProgressStateId: string,
    private readonly fetchImpl: typeof fetch = fetch,
  ) {}

  async nextTicketForSlot(slot: Slot): Promise<LinearIssue | null> {
    const query = /* GraphQL */ `
      query NextTicket($slot: String!) {
        issues(
          filter: {
            assignee: { name: { eq: $slot } }
            state: { type: { eq: "unstarted" } }
            labels: { name: { eq: "unblocked" } }
          }
          orderBy: createdAt
          first: 25
        ) {
          nodes {
            id
            identifier
            title
            priority
            state { id name type }
            assignee { id name }
            labels { nodes { name } }
          }
        }
      }
    `;

    const res = await this.fetchImpl(LINEAR_GQL_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: this.apiKey,
      },
      body: JSON.stringify({ query, variables: { slot } }),
    });
    if (!res.ok) {
      throw new Error(`Linear GraphQL ${res.status} ${res.statusText}`);
    }

    const json = (await res.json()) as LinearIssuesResponse;
    if (json.errors?.length) {
      throw new Error(
        `Linear GraphQL errors: ${json.errors.map((e) => e.message).join("; ")}`,
      );
    }

    const candidates = json.data?.issues?.nodes ?? [];
    return pickHighestPriority(candidates);
  }

  async markInProgress(issueId: string): Promise<void> {
    const mutation = /* GraphQL */ `
      mutation MarkInProgress($id: String!, $stateId: String!) {
        issueUpdate(id: $id, input: { stateId: $stateId }) { success }
      }
    `;
    const res = await this.fetchImpl(LINEAR_GQL_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: this.apiKey,
      },
      body: JSON.stringify({
        query: mutation,
        variables: { id: issueId, stateId: this.inProgressStateId },
      }),
    });
    if (!res.ok) {
      throw new Error(
        `Linear issueUpdate failed: ${res.status} ${res.statusText}`,
      );
    }
  }
}

/**
 * Sort Linear's `priority` (0 = none, 1 = urgent, 2 = high, 3 = medium, 4 = low)
 * such that urgent comes first, then 2/3/4, with 0 (none) last.
 */
export function pickHighestPriority(
  issues: readonly LinearIssue[],
): LinearIssue | null {
  if (issues.length === 0) return null;

  const score = (p: number): number => (p === 0 ? Number.POSITIVE_INFINITY : p);
  return [...issues].sort((a, b) => score(a.priority) - score(b.priority))[0]
    ?? null;
}
