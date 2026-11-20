import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";
import OcrOverlay from "./OcrOverlay";

const isOverlay = location.hash === "#ocr";
ReactDOM.createRoot(document.getElementById("root")).render(isOverlay ? <OcrOverlay /> : <App />);
