import { memo, useEffect, useRef, useState } from "react";
import type { Components } from "react-markdown";
import Markdown from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import { codeToHtml } from "shiki";
import { cn } from "@/lib/utils";
import "katex/dist/katex.min.css";

const BASE_MARKDOWN_CLASSES =
  "max-w-none break-words text-[13px] leading-6 text-[#e7dfd2] " +
  "[&_p]:my-0 [&_p+*]:mt-3 [&_ul+*]:mt-3 [&_ol+*]:mt-3 [&_blockquote+*]:mt-3 [&_pre+*]:mt-3 " +
  "[&_ul]:my-0 [&_ul]:list-disc [&_ul]:pl-5 [&_ul]:space-y-1.5 " +
  "[&_ol]:my-0 [&_ol]:list-decimal [&_ol]:pl-5 [&_ol]:space-y-1.5 " +
  "[&_li]:pl-1 [&_strong]:font-semibold [&_strong]:text-[#f2eadf] " +
  "[&_em]:text-[#d0c7bb] [&_hr]:my-4 [&_hr]:border-[#2b241d] " +
  "[&_a]:font-medium [&_a]:text-amber-300 [&_a]:underline [&_a]:decoration-amber-400/30 [&_a]:underline-offset-2 " +
  "[&_a:hover]:text-amber-200 " +
  "[&_blockquote]:border-l-2 [&_blockquote]:border-amber-500/30 [&_blockquote]:pl-4 [&_blockquote]:text-[#c7bcae] " +
  "[&_pre]:my-0 [&_pre]:overflow-x-auto [&_pre]:rounded-xl [&_pre]:border [&_pre]:border-[#302921] [&_pre]:bg-[#120f0d] [&_pre]:px-4 [&_pre]:py-3 " +
  "[&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_pre_code]:text-[12px] [&_pre_code]:leading-6 " +
  "[&_.shiki]:!bg-transparent [&_.shiki]:!p-0 " +
  "[&_.katex-display]:my-3 [&_.katex-display]:overflow-x-auto [&_.katex-display]:overflow-y-hidden";

const VARIANT_CLASSES = {
  bubble:
    "[&_:not(pre)>code]:rounded-md [&_:not(pre)>code]:border [&_:not(pre)>code]:border-amber-500/20 " +
    "[&_:not(pre)>code]:bg-amber-500/10 [&_:not(pre)>code]:px-1.5 [&_:not(pre)>code]:py-0.5 [&_:not(pre)>code]:text-[12px] [&_:not(pre)>code]:font-medium [&_:not(pre)>code]:text-amber-200",
  panel:
    "[&_:not(pre)>code]:rounded-md [&_:not(pre)>code]:border [&_:not(pre)>code]:border-[#373027] " +
    "[&_:not(pre)>code]:bg-[#171310] [&_:not(pre)>code]:px-1.5 [&_:not(pre)>code]:py-0.5 [&_:not(pre)>code]:text-[12px] [&_:not(pre)>code]:font-medium [&_:not(pre)>code]:text-amber-300",
} as const;

type ChatMarkdownVariant = keyof typeof VARIANT_CLASSES;

const LANGUAGE_MAP: Record<string, string> = {
  js: "javascript",
  ts: "typescript",
  py: "python",
  rb: "ruby",
  rs: "rust",
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  yml: "yaml",
  tf: "hcl",
  sol: "solidity",
  cs: "csharp",
  "c++": "cpp",
  "c#": "csharp",
  proto: "protobuf",
  gql: "graphql",
  md: "markdown",
  make: "makefile",
  tex: "latex",
};

function ShikiCode({ code, lang }: { code: string; lang: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [html, setHtml] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    codeToHtml(code, { lang, theme: "vitesse-dark" })
      .then((result) => {
        if (!cancelled) setHtml(result);
      })
      .catch(() => {
        if (!cancelled) setHtml(null);
      });
    return () => {
      cancelled = true;
    };
  }, [code, lang]);

  if (html) {
    return <div ref={ref} dangerouslySetInnerHTML={{ __html: html }} />;
  }
  return (
    <pre>
      <code>{code}</code>
    </pre>
  );
}

function CodeBlock({
  className,
  children,
  ...props
}: React.HTMLAttributes<HTMLElement> & { children?: React.ReactNode }) {
  const match = /language-(\w+)/.exec(className || "");
  const code = String(children).replace(/\n$/, "");

  if (!match) {
    return (
      <code className="rounded-md px-1.5 py-0.5 text-[12px] font-medium" {...props}>
        {children}
      </code>
    );
  }

  const raw = match[1].toLowerCase();
  const lang = LANGUAGE_MAP[raw] ?? raw;

  return <ShikiCode code={code} lang={lang} />;
}

function TableBlock({ children }: { children?: React.ReactNode }) {
  return (
    <div className="my-3 overflow-x-auto">
      <div className="min-w-[420px] overflow-hidden rounded-xl border border-[#2b241d] bg-[#151210] shadow-[inset_0_1px_0_rgba(255,255,255,0.02)]">
        <table className="w-full border-collapse text-[12px] leading-5">{children}</table>
      </div>
    </div>
  );
}

function TableHead({ children }: { children?: React.ReactNode }) {
  return <thead className="bg-[#1b1612] text-[#c9bcab]">{children}</thead>;
}

function TableRow({ children }: { children?: React.ReactNode }) {
  return <tr className="border-b border-[#2b241d] last:border-b-0">{children}</tr>;
}

function TableHeader({ children }: { children?: React.ReactNode }) {
  return (
    <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-[0.14em] text-[#9e907e]">
      {children}
    </th>
  );
}

function TableCell({ children }: { children?: React.ReactNode }) {
  return <td className="px-3 py-2 align-top text-[#e2d8cb]">{children}</td>;
}

const components: Components = {
  code: CodeBlock as Components["code"],
  table: TableBlock,
  thead: TableHead,
  tr: TableRow,
  th: TableHeader,
  td: TableCell,
};

const remarkPlugins = [remarkGfm, remarkMath];
const rehypePlugins = [rehypeKatex];

export const ChatMarkdown = memo(function ChatMarkdown({
  text,
  variant = "bubble",
}: {
  text: string;
  variant?: ChatMarkdownVariant;
}) {
  return (
    <div className={cn(BASE_MARKDOWN_CLASSES, VARIANT_CLASSES[variant])}>
      <Markdown remarkPlugins={remarkPlugins} rehypePlugins={rehypePlugins} components={components}>
        {text}
      </Markdown>
    </div>
  );
});
