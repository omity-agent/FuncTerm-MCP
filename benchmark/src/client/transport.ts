import {
  Client,
  StreamableHTTPClientTransport,
  type Transport as McpTransport,
} from "@modelcontextprotocol/client";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";
import { Effect } from "effect";

import { BenchmarkFailure, type Transport } from "../domain";
import type { Settings } from "../settings";

export type Peers = Record<Transport, Client>;
export function connectPeers(
  settings: Settings,
  environment: Record<string, string>,
  address: string,
) {
  const connect = (kind: Transport) =>
    Effect.gen(function* () {
      const transport: McpTransport =
        kind === "stdio"
          ? new StdioClientTransport({
              command: settings.executable,
              args: ["mcp", "--no-profile"],
              env: environment,
              stderr: "inherit",
            })
          : new StreamableHTTPClientTransport(
              new URL(`http://${address}/mcp`),
              {
                fetch: (input, init) => fetch(input, { ...init, proxy: "" }),
                reconnectionOptions: {
                  maxRetries: 0,
                  maxReconnectionDelay: 0,
                  initialReconnectionDelay: 0,
                  reconnectionDelayGrowFactor: 1,
                },
              },
            );
      const client = new Client({
        name: "functerm-benchmark",
        version: "1.0.0",
      });
      yield* Effect.acquireRelease(Effect.succeed(client), (peer) =>
        Effect.tryPromise({
          try: () => peer.close(),
          catch: (cause) => new BenchmarkFailure("transportFailed", cause),
        }).pipe(Effect.orDie),
      );
      yield* Effect.tryPromise({
        try: () => client.connect(transport),
        catch: (cause) => new BenchmarkFailure("transportFailed", cause),
      }).pipe(Effect.timeout(settings.timeouts.startupMilliseconds));
      return client;
    });
  return Effect.all(
    { stdio: connect("stdio"), http: connect("http") },
    {
      concurrency: "unbounded",
    },
  );
}
