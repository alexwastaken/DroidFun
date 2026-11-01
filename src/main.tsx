import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import Studio from "./Studio";
import "./styles.css";

const Root = import.meta.env.MODE === "studio" ? Studio : App;
createRoot(document.getElementById("root")!).render(<React.StrictMode><Root /></React.StrictMode>);
