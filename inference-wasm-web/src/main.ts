import { predict } from "./pkg/sonai";
import "./style.css";

const $input = document.getElementById("input") as HTMLTextAreaElement;
const $output = document.getElementById("output") as HTMLPreElement;

$input.addEventListener("input", () => {
  const input = $input.value;
  const start = performance.now();
  const { chance_ai, chance_human, metrics } = predict(input);
  const time = performance.now() - start;

  $output.innerText = `Text is most likely ${chance_ai >= chance_human ? "AI" : "Human"}

Chance:
  AI    = ${chance_ai.toFixed(2)}%
  Human = ${chance_human.toFixed(2)}%

Non-zero metrics:
${display(metrics)}

Time: ${time}ms`;
});

function display(metrics: Record<string, number>): string {
  const output = Object.entries(metrics)
    .filter(([, value]) => value !== 0)
    .sort(([, a], [, b]) => b - a)
    .map(([key, value]) =>
      Number.isInteger(value)
        ? `  ${key}: ${value}`
        : `  ${key}: ${value.toFixed(2)}`,
    );

  return output.join("\n");
}
