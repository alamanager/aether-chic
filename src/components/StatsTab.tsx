import { SpeedCard } from "@/components/SpeedCard";
import { TransferGraph } from "@/components/TransferGraph";

/** Stats tab: throughput + flow history. Same cards as before, new skin. */
export function StatsTab() {
  return (
    <div className="flex w-full flex-col items-center gap-3">
      <SpeedCard />
      <TransferGraph />
    </div>
  );
}
