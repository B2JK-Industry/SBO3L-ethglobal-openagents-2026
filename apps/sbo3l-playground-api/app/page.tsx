// Index page redirects users to the playground UI on the marketing
// site. This project is API-only; the visible UI lives at
// /playground/live on sbo3l-marketing.vercel.app.

import { redirect } from "next/navigation";

export default function Home(): never {
  redirect("https://sbo3l-marketing.vercel.app/playground/live");
}
