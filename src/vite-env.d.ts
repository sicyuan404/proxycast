/// <reference types="vite/client" />
/// <reference types="vite-plugin-svgr/client" />

// SVG 模块声明 - 支持 ?react 后缀导入为 React 组件
declare module "*.svg?react" {
  import * as React from "react";
  const ReactComponent: React.FunctionComponent<
    React.SVGProps<SVGSVGElement> & { title?: string }
  >;
  export default ReactComponent;
}

// 全局类型声明
declare global {
  type NotificationPermission = "default" | "denied" | "granted";

  interface Window {
    webkitAudioContext?: typeof AudioContext;
  }
}

export {};
