import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import WorkshopApp from "./WorkshopApp";
import "./styles.css";

const Root = import.meta.env.MODE === "studio" ? WorkshopApp : App;
createRoot(document.getElementById("root")!).render(<React.StrictMode><Root /></React.StrictMode>);
