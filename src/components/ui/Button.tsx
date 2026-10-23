import React from "react";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | "primary"
    | "primary-soft"
    | "secondary"
    | "warning"
    | "danger"
    | "danger-ghost"
    | "ghost";
  size?: "sm" | "md" | "lg";
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      children,
      className = "",
      variant = "primary",
      size = "md",
      // Only a button that asks for it submits the form it sits in.
      type = "button",
      ...props
    },
    ref,
  ) {
    // Hover arrives at once and fades out over 100 ms; a press scales to 0.97.
    const baseClasses =
      "inline-flex items-center justify-center gap-2 whitespace-nowrap font-medium border cursor-pointer select-none transition-[background-color,border-color,color,transform] duration-[var(--dur-1)] ease-[var(--ease-out)] hover:duration-0 active:scale-[0.97] active:duration-0 disabled:opacity-45 disabled:cursor-not-allowed disabled:active:scale-100";

    const variantClasses = {
      primary:
        "text-white bg-teal-fill border-transparent hover:bg-teal-fill-hover disabled:hover:bg-teal-fill",
      "primary-soft":
        "text-teal-700 bg-teal-50 border-transparent hover:bg-teal-100 disabled:hover:bg-teal-50",
      secondary:
        "text-text bg-surface border-line-strong shadow-[var(--shadow-1)] hover:bg-surface-2 hover:border-text-4 active:bg-surface-3 disabled:hover:bg-surface disabled:hover:border-line-strong",
      warning:
        "text-warn bg-warn-soft border-transparent hover:bg-warn hover:text-white disabled:hover:bg-warn-soft disabled:hover:text-warn",
      danger:
        "text-white bg-danger border-transparent hover:brightness-110 disabled:hover:brightness-100",
      "danger-ghost":
        "text-danger bg-transparent border-transparent hover:bg-danger-soft disabled:hover:bg-transparent",
      ghost:
        "text-text-2 bg-transparent border-transparent hover:bg-surface-2 hover:text-text active:bg-surface-3 disabled:hover:bg-transparent",
    };

    const sizeClasses = {
      sm: "h-[30px] px-2.5 text-[13px] rounded-[var(--r-sm)]",
      md: "h-9 px-3.5 text-sm rounded-[var(--r-md)]",
      lg: "h-10 px-[18px] text-[15px] rounded-[var(--r-md)]",
    };

    return (
      <button
        ref={ref}
        type={type}
        className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
        {...props}
      >
        {children}
      </button>
    );
  },
);
