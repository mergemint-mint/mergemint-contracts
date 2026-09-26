import React, { Suspense, useEffect, lazy } from 'react';
import { BrowserRouter, Link, Route, Routes, useLocation } from 'react-router-dom';
import { WalletProvider, useWallet } from './lib/WalletContext';
import { WalletConnectButton } from './components/WalletConnectButton';
import { NetworkMismatchBanner } from './components/NetworkMismatchBanner';
import { PageSkeleton } from './components/PageSkeleton';

// Lazy load page components to enable route-based code splitting
const BountyList = lazy(() => import('./pages/BountyList').then(m => ({ default: m.BountyList })));
const BountyDetail = lazy(() => import('./pages/BountyDetail').then(m => ({ default: m.BountyDetail })));
const CreateBounty = lazy(() => import('./pages/CreateBounty').then(m => ({ default: m.CreateBounty })));
const ContributorProfile = lazy(() => import('./pages/ContributorProfile').then(m => ({ default: m.ContributorProfile })));

function Nav() {
  const location = useLocation();
  const { address, connect, clearError } = useWallet();

  // A prior connect() failure otherwise stays visible until the next
  // connect() attempt, even after navigating away (issue #508).
  useEffect(() => {
    clearError();
  }, [location.pathname, clearError]);

  return (
    <nav>
      <Link to="/">Bounties</Link>
      <Link to="/create">Create Bounty</Link>
      <WalletConnectButton address={address} onConnect={connect} />
    </nav>
  );
}

export default function App() {
  return (
    <WalletProvider>
      <BrowserRouter>
        <Nav />
        <NetworkMismatchBanner />
        <Suspense fallback={<PageSkeleton />}>
          <Routes>
            <Route path="/" element={<BountyList />} />
            <Route path="/bounties/:id" element={<BountyDetail />} />
            <Route path="/create" element={<CreateBounty />} />
            <Route path="/contributors/:address" element={<ContributorProfile />} />
          </Routes>
        </Suspense>
      </BrowserRouter>
    </WalletProvider>
  );
}
