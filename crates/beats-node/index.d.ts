export declare function handlePlay(
  databaseUrl: string,
  cookieHeader: string | undefined | null,
  body: string,
): Promise<PlayResult>;

export declare function hello(name: string): string;

export interface PlayResult {
  status: number;
  revalidationTags: string[];
}

export declare function proxyDecision(pathname: string, cookieHeader?: string | undefined | null): ProxyResult;

export interface ProxyResult {
  redirectPathname?: string;
}
