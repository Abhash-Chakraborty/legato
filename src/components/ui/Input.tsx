import React from "react";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "compact";
}

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
  ({ className = "", variant = "default", disabled, ...props }, ref) => {
    return (
      <input
        ref={ref}
        className={`input ${variant === "compact" ? "compact" : ""} ${className}`}
        disabled={disabled}
        {...props}
      />
    );
  },
);
Input.displayName = "Input";
