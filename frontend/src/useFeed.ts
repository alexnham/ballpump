import { useEffect, useState } from "react";
import { demoSnapshot, snapshotSchema, type Snapshot } from "./data";
export function useFeed(demo: boolean) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(
    demo ? demoSnapshot() : null,
  );
  const [connection, setConnection] = useState("Connecting");
  useEffect(() => {
    let disposed = false;
    let socket: WebSocket | undefined;
    let retry: ReturnType<typeof setTimeout>;
    let attempts = 0;
    let last = 0;
    setSnapshot(demo ? demoSnapshot() : null);
    setConnection(demo ? "Demo" : "Connecting");
    if (demo) {
      const timer = setInterval(() => setSnapshot(demoSnapshot()), 1000);
      return () => clearInterval(timer);
    }
    const connect = () => {
      if (disposed) return;
      socket = new WebSocket(
        import.meta.env.VITE_WS_URL ||
          `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`,
      );
      socket.onopen = () => {
        attempts = 0;
        setConnection("Connected");
      };
      socket.onmessage = (event) => {
        try {
          if (typeof event.data !== "string" || event.data.length > 500_000)
            throw Error("Invalid message");
          const parsed = snapshotSchema.parse(JSON.parse(event.data));
          last = Date.now();
          setSnapshot(parsed);
          setConnection("Connected");
        } catch {
          setConnection("Invalid feed");
        }
      };
      socket.onerror = () => socket?.close();
      socket.onclose = () => {
        if (disposed) return;
        setConnection("Reconnecting");
        retry = setTimeout(
          connect,
          Math.min(15000, 1000 * 2 ** attempts++) + Math.random() * 400,
        );
      };
    };
    connect();
    const watchdog = setInterval(() => {
      if (last && Date.now() - last > 5000) setConnection("Stale");
      if (last && Date.now() - last > 15000) socket?.close();
    }, 1000);
    return () => {
      disposed = true;
      clearTimeout(retry);
      clearInterval(watchdog);
      socket?.close();
    };
  }, [demo]);
  return { snapshot, connection };
}
