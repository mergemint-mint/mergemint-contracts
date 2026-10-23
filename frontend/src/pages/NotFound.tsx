import { Link } from 'react-router-dom';

interface NotFoundProps {
  title?: string;
  message?: string;
}

export function NotFound({
  title = 'Page not found',
  message = "The page you're looking for doesn't exist or may have been moved.",
}: NotFoundProps) {
  return (
    <div className="flex min-h-[60vh] w-full flex-col items-center justify-center px-4 py-16 text-center sm:px-6 lg:px-8">
      <p className="text-6xl font-bold tracking-tight text-gray-900 sm:text-7xl">404</p>
      <h1 className="mt-4 text-2xl font-semibold text-gray-900 sm:text-3xl">
        {title}
      </h1>
      <p className="mt-3 max-w-md text-base text-gray-600">{message}</p>
      <div className="mt-8 flex flex-col gap-3 sm:flex-row">
        <Link
          to="/bounties"
          className="inline-flex items-center justify-center rounded-md bg-indigo-600 px-5 py-2.5 text-sm font-medium text-white transition-colors hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
        >
          Back to bounties
        </Link>
        <Link
          to="/"
          className="inline-flex items-center justify-center rounded-md border border-gray-300 bg-white px-5 py-2.5 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
        >
          Go home
        </Link>
      </div>
    </div>
  );
}

export default NotFound;
