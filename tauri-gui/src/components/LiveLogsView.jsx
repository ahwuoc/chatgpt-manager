import React from "react";
import { Button, Tag } from "antd";

export default function LiveLogsView({
  status,
  logs,
  setLogs,
  getLogColor,
  terminalEndRef,
}) {
  return (
    <div className="flex h-full flex-col gap-6 p-2">
      <div className="glass rounded-[24px] p-6 flex flex-col flex-1 min-h-[450px] overflow-hidden">
        {/* Logs Console Header */}
        <div className="flex flex-col md:flex-row md:items-center md:justify-between border-b border-white/5 pb-4 mb-4 gap-3">
          <div>
            <h3 className="text-lg font-bold text-white">Live Logger Terminal</h3>
            <p className="text-xs text-slate-400 mt-0.5">
              Realtime piped output từ các tiến trình automation và helper command.
            </p>
          </div>
          <div className="flex items-center gap-3 self-start md:self-auto">
            <Tag
              color={status === "running" ? "processing" : "default"}
              className="rounded-full px-3 py-0.5 uppercase tracking-wider text-[9px] font-bold border-none"
            >
              {status === "running" ? "Streaming" : "Idle"}
            </Tag>
            <Button
              size="small"
              type="text"
              onClick={() => setLogs([])}
              className="text-slate-400 hover:text-white text-xs"
            >
              Clear Terminal
            </Button>
          </div>
        </div>

        {/* Console Box */}
        <div className="flex-1 overflow-y-auto bg-black/45 p-5 rounded-2xl border border-white/5 font-mono text-xs leading-relaxed select-text thin-scrollbar min-h-[350px]">
          {logs.length === 0 ? (
            <span className="text-slate-500 italic block">
              // Logs console rỗng. Các tác vụ của bạn sẽ xuất hiện thời gian thực tại đây...
            </span>
          ) : (
            logs.map((log, index) => (
              <div key={index} className={`${getLogColor(log.text, log.level)} py-0.5`}>
                <span className="mr-2 text-slate-500 select-none">[{log.time}]</span>
                {log.text}
              </div>
            ))
          )}
          <div ref={terminalEndRef} />
        </div>
      </div>
    </div>
  );
}
