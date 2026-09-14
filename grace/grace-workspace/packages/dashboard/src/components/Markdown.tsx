import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { T } from "../theme";

function ToggleBox({ defaultChecked }: { defaultChecked: boolean }) {
  const [checked, setChecked] = useState(defaultChecked);
  return (
    <input
      type="checkbox"
      checked={checked}
      onChange={() => setChecked(!checked)}
      style={{
        width: 15,
        height: 15,
        marginRight: 8,
        cursor: "pointer",
        accentColor: T.accent,
        verticalAlign: "middle",
      }}
    />
  );
}

export function Markdown({ text }: { text: string }) {
  return (
    <div
      style={{
        fontSize: 14,
        color: T.text,
        lineHeight: 1.65,
      }}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          p: ({ children }) => (
            <p style={{ margin: "0 0 12px 0" }}>{children}</p>
          ),
          h1: ({ children }) => (
            <h1
              style={{
                fontSize: 20,
                fontWeight: 700,
                color: T.text,
                margin: "18px 0 10px",
              }}
            >
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2
              style={{
                fontSize: 17,
                fontWeight: 700,
                color: T.text,
                margin: "16px 0 8px",
              }}
            >
              {children}
            </h2>
          ),
          h3: ({ children }) => (
            <h3
              style={{
                fontSize: 15,
                fontWeight: 700,
                color: T.text,
                margin: "14px 0 6px",
              }}
            >
              {children}
            </h3>
          ),
          ul: ({ children }) => (
            <ul style={{ margin: "0 0 12px 0", paddingLeft: 22 }}>{children}</ul>
          ),
          ol: ({ children }) => (
            <ol style={{ margin: "0 0 12px 0", paddingLeft: 22 }}>{children}</ol>
          ),
          li: ({ children, className }) => (
            <li
              style={{
                marginBottom: 4,
                listStyleType:
                  className === "task-list-item" ? "none" : undefined,
                marginLeft: className === "task-list-item" ? -20 : undefined,
              }}
            >
              {children}
            </li>
          ),
          input: ({ type, checked, disabled: _disabled, ...rest }) => {
            if (type === "checkbox") {
              return <ToggleBox defaultChecked={!!checked} />;
            }
            return <input type={type} {...rest} />;
          },
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              style={{ color: T.accent, textDecoration: "underline" }}
            >
              {children}
            </a>
          ),
          code: ({ children, className }) => {
            const inline = !className;
            if (inline) {
              return (
                <code
                  style={{
                    background: T.codeBg,
                    padding: "1px 6px",
                    borderRadius: 4,
                    fontSize: 12,
                    fontFamily: "ui-monospace, SFMono-Regular, monospace",
                    color: T.accent,
                  }}
                >
                  {children}
                </code>
              );
            }
            return (
              <code
                style={{
                  fontFamily: "ui-monospace, SFMono-Regular, monospace",
                  fontSize: 12,
                }}
              >
                {children}
              </code>
            );
          },
          pre: ({ children }) => (
            <pre
              style={{
                background: T.codeBg,
                border: `1px solid ${T.border}`,
                borderRadius: 8,
                padding: 14,
                fontSize: 12,
                overflow: "auto",
                margin: "0 0 12px 0",
              }}
            >
              {children}
            </pre>
          ),
          blockquote: ({ children }) => (
            <blockquote
              style={{
                borderLeft: `3px solid ${T.accent}`,
                margin: "0 0 12px 0",
                paddingLeft: 14,
                color: T.textMuted,
                fontStyle: "italic",
              }}
            >
              {children}
            </blockquote>
          ),
          strong: ({ children }) => (
            <strong style={{ fontWeight: 700, color: T.text }}>{children}</strong>
          ),
          em: ({ children }) => <em style={{ fontStyle: "italic" }}>{children}</em>,
          hr: () => (
            <hr style={{ border: "none", borderTop: `1px solid ${T.border}`, margin: "16px 0" }} />
          ),
          table: ({ children }) => (
            <div style={{ overflowX: "auto", margin: "0 0 12px 0" }}>
              <table
                style={{
                  borderCollapse: "collapse",
                  fontSize: 12,
                  minWidth: "100%",
                }}
              >
                {children}
              </table>
            </div>
          ),
          th: ({ children }) => (
            <th
              style={{
                textAlign: "left",
                padding: "6px 10px",
                borderBottom: `1px solid ${T.border}`,
                fontWeight: 700,
              }}
            >
              {children}
            </th>
          ),
          td: ({ children }) => (
            <td
              style={{
                padding: "6px 10px",
                borderBottom: `1px solid ${T.border}`,
              }}
            >
              {children}
            </td>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
