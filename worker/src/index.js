const GOOGLE_TOKEN = "https://oauth2.googleapis.com/token";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method !== "POST") {
      return new Response("Method Not Allowed", { status: 405 });
    }

    const secret = request.headers.get("X-Proxy-Secret");
    if (!secret || secret !== env.PROXY_SECRET) {
      return new Response("Unauthorized", { status: 401 });
    }

    let body;
    try {
      body = await request.json();
    } catch {
      return new Response("Bad Request", { status: 400 });
    }

    let form;
    if (url.pathname === "/token") {
      if (!body.code || !body.code_verifier || !body.redirect_uri) {
        return new Response("Bad Request: missing fields", { status: 400 });
      }
      if (!body.redirect_uri.startsWith("http://127.0.0.1:")) {
        return new Response("Bad Request: invalid redirect_uri", { status: 400 });
      }
      form = new URLSearchParams({
        grant_type: "authorization_code",
        client_id: env.CLIENT_ID,
        client_secret: env.CLIENT_SECRET,
        code: body.code,
        code_verifier: body.code_verifier,
        redirect_uri: body.redirect_uri,
      });
    } else if (url.pathname === "/refresh") {
      if (!body.refresh_token) {
        return new Response("Bad Request: missing refresh_token", { status: 400 });
      }
      form = new URLSearchParams({
        grant_type: "refresh_token",
        client_id: env.CLIENT_ID,
        client_secret: env.CLIENT_SECRET,
        refresh_token: body.refresh_token,
      });
    } else {
      return new Response("Not Found", { status: 404 });
    }

    const response = await fetch(GOOGLE_TOKEN, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: form,
    });

    const data = await response.json();
    return Response.json(data, { status: response.status });
  },
};
