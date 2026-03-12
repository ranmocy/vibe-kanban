type RefreshStateCallback = (isRefreshing: boolean) => void;
type PauseableShape = { pause: () => void; resume: () => void };

class TokenManager {
  private subscribers = new Set<RefreshStateCallback>();

  /**
   * Get the current access token.
   * No auth needed for local mode — returns null.
   */
  async getToken(): Promise<string | null> {
    return null;
  }

  /**
   * Force a token refresh. No-op for local mode.
   */
  triggerRefresh(): Promise<string | null> {
    return Promise.resolve(null);
  }

  /**
   * Register an Electric shape for pause/resume during token refresh.
   * No-op for local mode. Returns an unsubscribe function.
   */
  registerShape(_shape: PauseableShape): () => void {
    return () => {};
  }

  /**
   * Get the current refreshing state synchronously.
   */
  getRefreshingState(): boolean {
    return false;
  }

  /**
   * Subscribe to refresh state changes.
   * Returns an unsubscribe function.
   */
  subscribe(callback: RefreshStateCallback): () => void {
    this.subscribers.add(callback);
    return () => this.subscribers.delete(callback);
  }
}

// Export singleton instance
export const tokenManager = new TokenManager();
