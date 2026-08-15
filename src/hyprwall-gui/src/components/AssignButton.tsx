interface Props {
  disabled: boolean;
  onClick: () => void;
}

export function AssignButton({ disabled, onClick }: Props) {
  return (
    <button onClick={onClick} disabled={disabled}>
      Assign wallpaper
    </button>
  );
}
