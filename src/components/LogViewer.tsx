import React from 'react';
import type { LogLine as LogLineType } from '../types/instance';

interface LogViewerProps {
  logs: LogLineType[];
}

export const LogViewer: React.FC<LogViewerProps> = ({ logs }) => {
  if (logs.length === 0) return null;

  return (
    <div className="logs">
      {logs.map((log, i) => (
        <div key={i} className={`log-line log-line--${log.level}`}>
          <span className="log-line__time">{log.timestamp}</span>
          <span className="log-line__text">{log.text}</span>
        </div>
      ))}
    </div>
  );
};
