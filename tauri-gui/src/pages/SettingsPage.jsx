import React from "react";
import SystemConfigView from "../components/SystemConfigView";

export default function SettingsPage({ consoleState }) {
  return (
    <SystemConfigView
      phone={consoleState.phone}
      setPhone={consoleState.setPhone}
      status={consoleState.status}
      autoPipeline={consoleState.autoPipeline}
      setAutoPipeline={consoleState.setAutoPipeline}
      handleSaveSettings={consoleState.handleSaveSettings}
    />
  );
}
