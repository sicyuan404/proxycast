/**
 * I18nPatchProvider Component
 *
 * React Provider component that manages the i18n patch state.
 * Applies DOM text replacement when language changes and watches for
 * dynamic content via MutationObserver.
 *
 * This is the core of the Patch Layer architecture - it intercepts
 * text rendering and applies translations without modifying original components.
 */

/* eslint-disable react-refresh/only-export-components */
import {
  useEffect,
  useState,
  createContext,
  useContext,
  ReactNode,
} from "react";
import { replaceTextInDOM } from "./dom-replacer";
import { Language, isValidLanguage } from "./text-map";

export interface I18nPatchContextValue {
  language: Language;
  setLanguage: (lang: Language) => void;
}

const I18nPatchContext = createContext<I18nPatchContextValue>({
  language: "zh",
  setLanguage: () => {},
});

/**
 * Hook to access i18n patch context
 * Must be used within I18nPatchProvider
 */
export const useI18nPatch = () => {
  const context = useContext(I18nPatchContext);
  if (!context) {
    throw new Error("useI18nPatch must be used within I18nPatchProvider");
  }
  return context;
};

export interface I18nPatchProviderProps {
  children: ReactNode;
  initialLanguage?: Language;
}

/**
 * I18nPatchProvider Component
 *
 * Provides i18n context and manages DOM text replacement.
 * Automatically patches new content via MutationObserver.
 */
export function I18nPatchProvider({
  children,
  initialLanguage = "zh",
}: I18nPatchProviderProps) {
  const [language, setLanguage] = useState<Language>(initialLanguage);

  // Validate and normalize language
  const normalizeLanguage = (lang: string): Language => {
    if (isValidLanguage(lang)) {
      return lang;
    }
    console.warn(`[i18n] Invalid language "${lang}", falling back to "zh"`);
    return "zh";
  };

  // Handle language change
  const handleSetLanguage = (lang: Language) => {
    const normalized = normalizeLanguage(lang);
    setLanguage(normalized);
  };

  useEffect(() => {
    // Apply patches when language changes
    replaceTextInDOM(language);

    // Track language changes
    if (window.__I18N_METRICS__) {
      window.__I18N_METRICS__.languageChanges++;
    }

    // Set up MutationObserver for dynamic content with debouncing
    let timeoutId: number | null = null;
    const observer = new MutationObserver((mutations) => {
      // 收集需要处理的新增节点
      const rootsToProcess = new Set<Element>();

      mutations.forEach((mutation) => {
        const target = mutation.target as HTMLElement;

        // 忽略 input、textarea 内部的变化
        if (
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.closest("input, textarea")
        ) {
          return;
        }

        // 忽略已经打过补丁的节点的属性变化
        if (
          mutation.type === "attributes" &&
          target instanceof Element &&
          target.hasAttribute("data-i18n-patched")
        ) {
          return;
        }

        // 收集新增的节点
        if (mutation.type === "childList") {
          mutation.addedNodes.forEach((node) => {
            if (node instanceof Element) {
              rootsToProcess.add(node);
            } else if (node instanceof Text && node.parentElement) {
              rootsToProcess.add(node.parentElement);
            }
          });
        } else if (mutation.type === "characterData") {
          // 文本内容变化，处理父元素
          if (target.parentElement) {
            rootsToProcess.add(target.parentElement);
          }
        }
      });

      // 如果没有需要处理的节点，直接返回
      if (rootsToProcess.size === 0) return;

      // 防抖：延迟 300ms 执行，避免频繁触发
      if (timeoutId !== null) {
        clearTimeout(timeoutId);
      }
      timeoutId = window.setTimeout(() => {
        // 只处理新增的节点子树，而不是整个文档
        rootsToProcess.forEach((root) => {
          replaceTextInDOM(language, root);
        });
        timeoutId = null;
      }, 300);
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: false, // 不监听属性变化，减少触发频率
    });

    return () => {
      observer.disconnect();
      if (timeoutId !== null) {
        clearTimeout(timeoutId);
      }
    };
  }, [language]);

  return (
    <I18nPatchContext.Provider
      value={{ language, setLanguage: handleSetLanguage }}
    >
      {children}
    </I18nPatchContext.Provider>
  );
}
