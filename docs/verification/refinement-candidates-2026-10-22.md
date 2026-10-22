# Source-bound refinement options

The isolated worker now asks the model to select from exact Rust-generated
options for hesitation removal, explicit corrections and sentence endpoints.
Additional punctuation proposals still pass the same complete text validator.
A model answer must contain one boolean decision for every option; incomplete
lists, unknown fields and invalid combined edits fail as a whole. No option is
applied merely because Rust generated it.

## Executed software checks

Ten new core tests cover exact source spans, quoted text and links, adjacent
hesitations, grammatical comma alternatives, complete decimal and signed values,
terminal punctuation, sentence endpoints and candidate-count limits. Four worker
selection/rendering tests cover complete decisions and atomic rejection.

Independent review found clipped numeric boundaries, an invalid comma
alternative discarding a valid deletion, and a duplicate consuming the final
candidate slot. Each was reproduced by a failing regression and corrected.
The final targeted review found no remaining concrete issue in those changes.
These checks establish candidate mechanics, not model judgment.

## Model-quality limitation

The model still sometimes chooses only one of two required hesitation removals.
For `Um, please send, uh, the report today.`, the diagnostic result removed `Um,`
but retained `uh`. The full rendered prompt contained both options; it was not
truncated. Changing the prefill chunk from 256 to 2048 tokens did not change this
result. Temporary diagnostic output was removed from the source afterward.

The same rendered prompt and pinned model assets also produced this incomplete
selection with the official `mlx-lm` 0.32.2 / MLX 0.32.4 runtime. This reproduces
the symptom outside the Rust worker; it is not a general numerical-parity proof
or performance comparison. The reference invocation used offline local assets,
greedy sampling and no chat-template transformation of the rendered prompt.
See the [upstream runtime documentation](https://github.com/ml-explore/mlx-lm).

A separate shorter, grouped-choice prompt also missed a required removal. More
prompt examples alone have not established reliable cleanup. Model-quality
acceptance, application integration and package acceptance remain open. The
existing ASR application has not been switched to this experimental flow.

## Separating model context from validation anchors

The later renderer keeps the full validation anchors in Rust but omits them
from model options. Repeated source spans are instead identified by their
one-based occurrence, counting overlapping matches. Renderer tests cover both
ordinary repetition and overlapping source matches.

In the controlled two-hesitation diagnostic, this change was sufficient to
produce both required removals. It also removed both hesitations in the German
case from the earlier corpus. A targeted 17-case run nevertheless produced ten
rejected responses and seven accepted responses; two accepted responses still
missed a required correction. Quality acceptance therefore remains open. This
is evidence of a specific prompt-representation improvement, not a claim that
all model errors share the same cause.

The isolated worker uses prompt/input version 6 and policy version 2 for this
representation. Original validation and atomic acceptance are unchanged.
