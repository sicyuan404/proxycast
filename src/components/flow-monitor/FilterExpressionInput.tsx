import React, { useState, useEffect, useRef, useCallback } from "react";
import { safeInvoke } from "@/lib/dev-bridge";
import { Search, AlertCircle, CheckCircle2, HelpCircle, X } from "lucide-react";
import { cn } from "@/lib/utils";

// ============================================================================
// 类型定义
// ============================================================================

/**
 * 过滤表达式解析结果
 */
interface ParseFilterResult {
  valid: boolean;
  error: string | null;
  expr: unknown | null;
}

/**
 * 过滤表达式帮助项
 */
export interface FilterHelpItem {
  syntax: string;
  description: string;
}

/**
 * 自动补全建议
 */
interface AutocompleteSuggestion {
  text: string;
  description: string;
  type: "filter" | "operator" | "value";
}

interface FilterExpressionInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (expression: string) => void;
  onValidationChange?: (valid: boolean, error: string | null) => void;
  placeholder?: string;
  className?: string;
  showHelp?: boolean;
  onHelpToggle?: () => void;
}

// ============================================================================
// 过滤器定义（用于语法高亮和自动补全）
// ============================================================================

const FILTER_KEYWORDS = [
  { prefix: "~m", name: "model", hasArg: true, description: "模型名称匹配" },
  { prefix: "~p", name: "provider", hasArg: true, description: "提供商匹配" },
  { prefix: "~s", name: "state", hasArg: true, description: "状态匹配" },
  { prefix: "~e", name: "error", hasArg: false, description: "有错误" },
  { prefix: "~t", name: "toolcalls", hasArg: false, description: "有工具调用" },
  { prefix: "~k", name: "thinking", hasArg: false, description: "有思维链" },
  { prefix: "~starred", name: "starred", hasArg: false, description: "已收藏" },
  { prefix: "~tag", name: "tag", hasArg: true, description: "包含标签" },
  { prefix: "~b", name: "body", hasArg: true, description: "内容匹配" },
  {
    prefix: "~bq",
    name: "bodyrequest",
    hasArg: true,
    description: "请求内容匹配",
  },
  {
    prefix: "~bs",
    name: "bodyresponse",
    hasArg: true,
    description: "响应内容匹配",
  },
  {
    prefix: "~tokens",
    name: "tokens",
    hasArg: true,
    description: "Token 数量比较",
  },
  {
    prefix: "~latency",
    name: "latency",
    hasArg: true,
    description: "延迟比较",
  },
];

const OPERATORS = [
  { symbol: "&", description: "AND 逻辑" },
  { symbol: "|", description: "OR 逻辑" },
  { symbol: "!", description: "NOT 逻辑" },
  { symbol: "(", description: "左括号" },
  { symbol: ")", description: "右括号" },
];

const STATE_VALUES = [
  "pending",
  "streaming",
  "completed",
  "failed",
  "cancelled",
];

const COMPARISON_OPS = [">", ">=", "<", "<=", "="];

// ============================================================================
// 组件实现
// ============================================================================

export function FilterExpressionInput({
  value,
  onChange,
  onSubmit,
  onValidationChange,
  placeholder = "输入过滤表达式，如 ~m claude & ~p kiro",
  className,
  showHelp = false,
  onHelpToggle,
}: FilterExpressionInputProps) {
  const [isValid, setIsValid] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [suggestions, setSuggestions] = useState<AutocompleteSuggestion[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [selectedSuggestionIndex, setSelectedSuggestionIndex] = useState(0);
  const validationTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );

  const inputRef = useRef<HTMLInputElement>(null);
  const suggestionsRef = useRef<HTMLDivElement>(null);

  // 验证表达式
  const validateExpression = useCallback(
    async (expr: string) => {
      if (!expr.trim()) {
        setIsValid(null);
        setError(null);
        onValidationChange?.(true, null);
        return;
      }

      try {
        const result = await safeInvoke<ParseFilterResult>("parse_filter", {
          expression: expr,
        });

        setIsValid(result.valid);
        setError(result.error);
        onValidationChange?.(result.valid, result.error);
      } catch (e) {
        setIsValid(false);
        const errorMsg = e instanceof Error ? e.message : "验证失败";
        setError(errorMsg);
        onValidationChange?.(false, errorMsg);
      }
    },
    [onValidationChange],
  );

  // 防抖验证
  useEffect(() => {
    if (validationTimeoutRef.current) {
      clearTimeout(validationTimeoutRef.current);
    }

    const timeout = setTimeout(() => {
      validateExpression(value);
    }, 300);

    validationTimeoutRef.current = timeout;

    return () => {
      if (timeout) {
        clearTimeout(timeout);
      }
    };
  }, [value, validateExpression]);

  // 生成自动补全建议
  const generateSuggestions = useCallback(
    (input: string, cursorPos: number) => {
      const textBeforeCursor = input.slice(0, cursorPos);
      const lastToken = textBeforeCursor.split(/[\s&|!()]+/).pop() || "";

      const newSuggestions: AutocompleteSuggestion[] = [];

      // 如果以 ~ 开头，建议过滤器
      if (lastToken.startsWith("~")) {
        const filterPrefix = lastToken.toLowerCase();
        FILTER_KEYWORDS.forEach((filter) => {
          if (filter.prefix.toLowerCase().startsWith(filterPrefix)) {
            newSuggestions.push({
              text: filter.prefix,
              description: filter.description,
              type: "filter",
            });
          }
        });
      }
      // 如果刚输入了 ~s，建议状态值
      else if (/~s\s*$/.test(textBeforeCursor)) {
        STATE_VALUES.forEach((state) => {
          newSuggestions.push({
            text: state,
            description: `状态: ${state}`,
            type: "value",
          });
        });
      }
      // 如果刚输入了 ~tokens 或 ~latency，建议比较运算符
      else if (/~(tokens|latency)\s*$/.test(textBeforeCursor)) {
        COMPARISON_OPS.forEach((op) => {
          newSuggestions.push({
            text: op,
            description: `比较运算符: ${op}`,
            type: "operator",
          });
        });
      }
      // 如果输入为空或刚输入了运算符，建议过滤器
      else if (!lastToken || /[&|!()]$/.test(textBeforeCursor.trim())) {
        FILTER_KEYWORDS.slice(0, 6).forEach((filter) => {
          newSuggestions.push({
            text: filter.prefix,
            description: filter.description,
            type: "filter",
          });
        });
      }
      // 如果刚输入了过滤器值，建议逻辑运算符
      else if (lastToken && !lastToken.startsWith("~")) {
        OPERATORS.slice(0, 3).forEach((op) => {
          newSuggestions.push({
            text: op.symbol,
            description: op.description,
            type: "operator",
          });
        });
      }

      setSuggestions(newSuggestions);
      setShowSuggestions(newSuggestions.length > 0);
      setSelectedSuggestionIndex(0);
    },
    [],
  );

  // 处理输入变化
  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = e.target.value;
    onChange(newValue);
    generateSuggestions(newValue, e.target.selectionStart || newValue.length);
  };

  // 处理键盘事件
  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (showSuggestions && suggestions.length > 0) {
      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          setSelectedSuggestionIndex((prev) =>
            prev < suggestions.length - 1 ? prev + 1 : 0,
          );
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedSuggestionIndex((prev) =>
            prev > 0 ? prev - 1 : suggestions.length - 1,
          );
          break;
        case "Tab":
        case "Enter":
          if (showSuggestions && suggestions[selectedSuggestionIndex]) {
            e.preventDefault();
            applySuggestion(suggestions[selectedSuggestionIndex]);
          } else if (e.key === "Enter" && isValid !== false) {
            e.preventDefault();
            onSubmit(value);
          }
          break;
        case "Escape":
          e.preventDefault();
          setShowSuggestions(false);
          break;
      }
    } else if (e.key === "Enter" && isValid !== false) {
      e.preventDefault();
      onSubmit(value);
    }
  };

  // 应用建议
  const applySuggestion = (suggestion: AutocompleteSuggestion) => {
    const input = inputRef.current;
    if (!input) return;

    const cursorPos = input.selectionStart || value.length;
    const textBeforeCursor = value.slice(0, cursorPos);
    const textAfterCursor = value.slice(cursorPos);

    // 找到最后一个 token 的开始位置
    const lastTokenMatch = textBeforeCursor.match(/[~\w\-.*]+$/);
    const lastTokenStart = lastTokenMatch
      ? cursorPos - lastTokenMatch[0].length
      : cursorPos;

    // 构建新值
    const newValue =
      value.slice(0, lastTokenStart) +
      suggestion.text +
      (suggestion.type === "filter" &&
      FILTER_KEYWORDS.find((f) => f.prefix === suggestion.text)?.hasArg
        ? " "
        : "") +
      textAfterCursor;

    onChange(newValue);
    setShowSuggestions(false);

    // 设置光标位置
    setTimeout(() => {
      const newCursorPos =
        lastTokenStart +
        suggestion.text.length +
        (suggestion.type === "filter" &&
        FILTER_KEYWORDS.find((f) => f.prefix === suggestion.text)?.hasArg
          ? 1
          : 0);
      input.setSelectionRange(newCursorPos, newCursorPos);
      input.focus();
    }, 0);
  };

  // 点击外部关闭建议
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        suggestionsRef.current &&
        !suggestionsRef.current.contains(e.target as Node) &&
        inputRef.current &&
        !inputRef.current.contains(e.target as Node)
      ) {
        setShowSuggestions(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // 渲染语法高亮的文本
  const renderHighlightedText = () => {
    if (!value) return null;

    const parts: React.ReactNode[] = [];
    let remaining = value;
    let key = 0;

    while (remaining.length > 0) {
      let matched = false;

      // 匹配过滤器
      for (const filter of FILTER_KEYWORDS) {
        const regex = new RegExp(
          `^(${filter.prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})(?:\\s+([^&|!()]+))?`,
          "i",
        );
        const match = remaining.match(regex);
        if (match) {
          parts.push(
            <span
              key={key++}
              className="text-blue-600 dark:text-blue-400 font-medium"
            >
              {match[1]}
            </span>,
          );
          if (match[2]) {
            parts.push(
              <span key={key++} className="text-green-600 dark:text-green-400">
                {" "}
                {match[2]}
              </span>,
            );
          }
          remaining = remaining.slice(match[0].length);
          matched = true;
          break;
        }
      }

      if (!matched) {
        // 匹配运算符
        const opMatch = remaining.match(/^([&|!()])/);
        if (opMatch) {
          parts.push(
            <span
              key={key++}
              className="text-purple-600 dark:text-purple-400 font-bold"
            >
              {opMatch[1]}
            </span>,
          );
          remaining = remaining.slice(1);
          matched = true;
        }
      }

      if (!matched) {
        // 匹配空白
        const wsMatch = remaining.match(/^(\s+)/);
        if (wsMatch) {
          parts.push(<span key={key++}>{wsMatch[1]}</span>);
          remaining = remaining.slice(wsMatch[1].length);
          matched = true;
        }
      }

      if (!matched) {
        // 其他字符
        parts.push(<span key={key++}>{remaining[0]}</span>);
        remaining = remaining.slice(1);
      }
    }

    return parts;
  };

  return (
    <div className={cn("relative", className)}>
      {/* 输入框容器 */}
      <div className="relative">
        {/* 语法高亮层 */}
        <div
          className="absolute inset-0 px-9 py-2 text-sm pointer-events-none whitespace-pre overflow-hidden"
          aria-hidden="true"
        >
          {renderHighlightedText()}
        </div>

        {/* 实际输入框 */}
        <div className="relative flex items-center">
          <Search className="absolute left-3 h-4 w-4 text-muted-foreground" />
          <input
            ref={inputRef}
            type="text"
            value={value}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            onFocus={() => generateSuggestions(value, value.length)}
            placeholder={placeholder}
            className={cn(
              "w-full rounded-lg border bg-transparent pl-9 pr-20 py-2 text-sm text-transparent caret-foreground",
              "focus:outline-none focus:ring-2 focus:ring-primary",
              isValid === false && "border-red-500 focus:ring-red-500",
              isValid === true &&
                value &&
                "border-green-500 focus:ring-green-500",
            )}
            spellCheck={false}
            autoComplete="off"
          />

          {/* 右侧图标 */}
          <div className="absolute right-2 flex items-center gap-1">
            {/* 验证状态图标 */}
            {value && isValid === true && (
              <CheckCircle2 className="h-4 w-4 text-green-500" />
            )}
            {value && isValid === false && (
              <AlertCircle className="h-4 w-4 text-red-500" />
            )}

            {/* 清除按钮 */}
            {value && (
              <button
                onClick={() => {
                  onChange("");
                  inputRef.current?.focus();
                }}
                className="p-1 hover:bg-muted rounded"
                title="清除"
              >
                <X className="h-3 w-3 text-muted-foreground" />
              </button>
            )}

            {/* 帮助按钮 */}
            {onHelpToggle && (
              <button
                onClick={onHelpToggle}
                className={cn(
                  "p-1 hover:bg-muted rounded",
                  showHelp && "bg-muted",
                )}
                title="显示帮助"
              >
                <HelpCircle className="h-4 w-4 text-muted-foreground" />
              </button>
            )}
          </div>
        </div>
      </div>

      {/* 错误提示 */}
      {error && (
        <div className="mt-1 text-xs text-red-500 flex items-center gap-1">
          <AlertCircle className="h-3 w-3" />
          {error}
        </div>
      )}

      {/* 自动补全建议 */}
      {showSuggestions && suggestions.length > 0 && (
        <div
          ref={suggestionsRef}
          className="absolute z-50 mt-1 w-full max-h-60 overflow-auto rounded-lg border bg-card shadow-lg"
        >
          {suggestions.map((suggestion, index) => (
            <button
              key={`${suggestion.text}-${index}`}
              onClick={() => applySuggestion(suggestion)}
              className={cn(
                "w-full px-3 py-2 text-left text-sm flex items-center justify-between",
                "hover:bg-muted",
                index === selectedSuggestionIndex && "bg-muted",
              )}
            >
              <span
                className={cn(
                  "font-mono",
                  suggestion.type === "filter" &&
                    "text-blue-600 dark:text-blue-400",
                  suggestion.type === "operator" &&
                    "text-purple-600 dark:text-purple-400",
                  suggestion.type === "value" &&
                    "text-green-600 dark:text-green-400",
                )}
              >
                {suggestion.text}
              </span>
              <span className="text-xs text-muted-foreground">
                {suggestion.description}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export default FilterExpressionInput;
