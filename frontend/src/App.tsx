import { Link, Route, Routes } from "react-router-dom";
import { Compass } from "lucide-react";
import { AppShell } from "@/dashboard/AppShell";
import { Overview } from "@/pages/Overview";
import { NewExtraction } from "@/pages/NewExtraction";
import { Jobs } from "@/pages/Jobs";
import { Results } from "@/pages/Results";
import { McpConfig } from "@/pages/McpConfig";
import { ApiKeys } from "@/pages/ApiKeys";
import { Settings } from "@/pages/Settings";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/skeleton";

function NotFound() {
  return (
    <EmptyState
      icon={<Compass className="size-6" />}
      title="Page not found"
      description="The route you requested does not exist in this dashboard."
      action={
        <Button asChild variant="outline">
          <Link to="/">Back to overview</Link>
        </Button>
      }
    />
  );
}

export function App() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<Overview />} />
        <Route path="extract" element={<NewExtraction />} />
        <Route path="jobs" element={<Jobs />} />
        <Route path="results" element={<Results />} />
        <Route path="mcp" element={<McpConfig />} />
        <Route path="api-keys" element={<ApiKeys />} />
        <Route path="settings" element={<Settings />} />
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}
