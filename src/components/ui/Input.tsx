import React from "react";

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  variant?: "default" | "compact";
}

export const Input: React.FC<InputProps> = ({
  className = "",
  variant = "default",
  disabled,
  ...props
}) => {
  return (
    <input
      className={`input ${variant === "compact" ? "compact" : ""} ${className}`}
      disabled={disabled}
      {...props}
    />
  );
};
