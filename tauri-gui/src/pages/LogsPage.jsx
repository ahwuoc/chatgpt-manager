import React from "react";
import LiveLogsView from "../components/LiveLogsView";

export default function LogsPage({ consoleState }) {
  return (
    <LiveLogsView
      status={consoleState.status}
      logs={consoleState.logs}
      setLogs={consoleState.setLogs}
      getLogColor={consoleState.getLogColor}
      terminalEndRef={consoleState.terminalEndRef}
    />
  );
}
