import { safeInvoke } from "@/lib/dev-bridge";

export interface RouteEndpoint {
  path: string;
  protocol: string;
  url: string;
}

export interface RouteInfo {
  selector: string;
  provider_type: string;
  credential_count: number;
  endpoints: RouteEndpoint[];
  tags: string[];
  enabled: boolean;
}

export interface RouteListResponse {
  base_url: string;
  default_provider: string;
  routes: RouteInfo[];
}

export interface CurlExample {
  description: string;
  command: string;
}

export const routesApi = {
  async getAvailableRoutes(): Promise<RouteListResponse> {
    return safeInvoke("get_available_routes");
  },

  async getCurlExamples(selector: string): Promise<CurlExample[]> {
    return safeInvoke("get_route_curl_examples", { selector });
  },
};
