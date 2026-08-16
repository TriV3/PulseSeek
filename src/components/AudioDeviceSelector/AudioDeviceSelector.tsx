import type { DeviceInfoData } from "../../api/commandEnvelope";

interface AudioDeviceSelectorProps {
  devices: DeviceInfoData[];
  selectedDeviceId: string | null;
  isLoading: boolean;
  isSelecting: boolean;
  error: string | null;
  onChange: (deviceId: string) => void | Promise<void>;
  onRetry: () => void | Promise<void>;
}

export function AudioDeviceSelector({
  devices,
  selectedDeviceId,
  isLoading,
  isSelecting,
  error,
  onChange,
  onRetry,
}: AudioDeviceSelectorProps) {
  const disabled = isLoading || isSelecting || devices.length === 0;
  const statusId = "audio-output-device-status";
  return (
    <section aria-label="Audio output" aria-busy={isLoading || isSelecting}>
      <label htmlFor="audio-output-device">Output device</label>
      <select
        id="audio-output-device"
        value={selectedDeviceId ?? ""}
        disabled={disabled}
        aria-describedby={error ? statusId : undefined}
        onChange={(event) => void onChange(event.target.value)}
      >
        {devices.length === 0 ? (
          <option value="">No output devices</option>
        ) : null}
        {devices.map((device) => (
          <option key={device.id} value={device.id}>
            {device.name}
          </option>
        ))}
      </select>
      {error ? (
        <p id={statusId} role="alert">
          {error}
        </p>
      ) : null}
      {isLoading ? (
        <p id={statusId} role="status">
          Loading output devices…
        </p>
      ) : null}
      {(error || devices.length === 0) && !isLoading ? (
        <button type="button" onClick={onRetry}>
          Retry
        </button>
      ) : null}
    </section>
  );
}
