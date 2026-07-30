import { useCallback, useEffect, useRef, useState } from "react";
import {
  currentDevice,
  listDevices,
  selectDevice,
  type DeviceInfoData,
} from "../api/commandEnvelope";
import { onDeviceLost } from "../api/playbackEvents";

export function useAudioDevices() {
  const [devices, setDevices] = useState<DeviceInfoData[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSelecting, setIsSelecting] = useState(false);
  const refreshGeneration = useRef(0);
  const selectionGeneration = useRef(0);
  const mounted = useRef(false);

  const refresh = useCallback(async () => {
    const generation = ++refreshGeneration.current;
    setIsLoading(true);
    setError(null);
    try {
      const [available, current] = await Promise.all([
        listDevices(),
        currentDevice(),
      ]);
      if (generation !== refreshGeneration.current || !mounted.current) return;
      setDevices(available);
      setSelectedDeviceId(current?.id ?? null);
    } catch (cause: unknown) {
      if (generation === refreshGeneration.current && mounted.current) {
        setError(
          cause instanceof Error ? cause.message : "Audio devices unavailable.",
        );
      }
    } finally {
      if (generation === refreshGeneration.current && mounted.current)
        setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void Promise.resolve().then(refresh);
    let unlisten: (() => void) | undefined;
    void Promise.resolve(
      onDeviceLost(() => {
        if (!mounted.current) return;
        setError("Output device lost.");
        void refresh();
      }),
    )
      .then((cleanup) => {
        if (!mounted.current) cleanup();
        else unlisten = cleanup;
      })
      .catch(() => undefined);
    return () => {
      mounted.current = false;
      unlisten?.();
    };
  }, [refresh]);

  const choose = async (deviceId: string) => {
    const generation = ++selectionGeneration.current;
    const previous = selectedDeviceId;
    setSelectedDeviceId(deviceId);
    setIsSelecting(true);
    setError(null);
    try {
      await selectDevice(deviceId);
      const confirmed = await currentDevice();
      if (generation !== selectionGeneration.current || !mounted.current)
        return;
      setSelectedDeviceId(confirmed?.id ?? null);
      await refresh();
    } catch (cause: unknown) {
      if (generation === selectionGeneration.current && mounted.current) {
        setSelectedDeviceId(previous);
        setError(
          cause instanceof Error
            ? cause.message
            : "Could not select output device.",
        );
      }
    } finally {
      if (generation === selectionGeneration.current && mounted.current)
        setIsSelecting(false);
    }
  };

  return {
    devices,
    selectedDeviceId,
    error,
    isLoading,
    isSelecting,
    choose,
    refresh,
  };
}
