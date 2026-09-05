// Hierarki error HTTP kecil — dilempar dari `*.service.ts` (yang TIDAK
// tahu soal Express), ditangkap `middleware/errorHandler.ts` (yang
// tahu cara ubahnya jadi response JSON dengan status code yang benar).
// Ini SATU-SATUNYA cara service melaporkan kegagalan yang harus
// nyampe ke client sebagai response tertentu (bukan 500 generic).

export class HttpError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "HttpError";
  }
}

export class BadRequestError extends HttpError {
  constructor(message = "Request tidak valid") {
    super(400, message);
  }
}

export class UnauthorizedError extends HttpError {
  constructor(message = "Tidak terautentikasi") {
    super(401, message);
  }
}

export class ForbiddenError extends HttpError {
  constructor(message = "Tidak diizinkan") {
    super(403, message);
  }
}

export class NotFoundError extends HttpError {
  constructor(message = "Tidak ditemukan") {
    super(404, message);
  }
}

export class ConflictError extends HttpError {
  constructor(message = "Konflik data") {
    super(409, message);
  }
}
