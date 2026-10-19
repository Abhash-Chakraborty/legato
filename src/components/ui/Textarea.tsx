import React from "react";

interface TextareaProps
  extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  variant?: "default" | "compact";
}

export const Textarea: React.FC<TextareaProps> = ({
  className = "",
  variant = "default",
  ...props
}) => {
  return (
    <textarea
      className={`input ${variant === "compact" ? "min-h-[80px]" : ""} ${className}`}
      {...props}
    />
  );
};
