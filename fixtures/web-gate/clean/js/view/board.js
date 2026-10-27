import { formatCount } from "../logic/format.js";

const board = document.querySelector("#board");
if (board) {
  board.textContent = formatCount(1);
}
