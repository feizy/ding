import React from 'react';
import type { DingStatus } from '../types/instance';

interface StatusDotProps {
  status: DingStatus;
  size?: 'normal' | 'small';
  className?: string;
}

export const StatusDot: React.FC<StatusDotProps> = ({ status, size = 'normal', className = '' }) => {
  const baseClass = size === 'small' ? 'dot-small' : 'status-dot';
  return <div className={`${baseClass} ${baseClass}--${status} ${className}`} />;
};
