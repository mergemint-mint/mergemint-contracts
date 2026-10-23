import React, { useEffect, useState } from 'react';

/**
 * Dismissible onboarding tour for first-time contributors (issue #1084)
 */
export function OnboardingTour() {
  const [isVisible, setIsVisible] = useState(false);
  const [step, setStep] = useState(0);

  useEffect(() => {
    const hasSeenTour = localStorage.getItem('onboarding-tour-completed');
    if (!hasSeenTour) {
      setIsVisible(true);
    }
  }, []);

  const steps = [
    {
      title: 'Welcome to MergeMint',
      description: 'Get paid for completing bounties on the Stellar blockchain.',
    },
    {
      title: 'Claim a Bounty',
      description: 'Browse available bounties and click "Claim" to start working on one.',
    },
    {
      title: 'Submit Your Work',
      description: 'When done, submit your work with a link to your deliverable.',
    },
    {
      title: 'Get Paid Instantly',
      description: 'Once approved, payment is transferred to your Stellar account automatically.',
    },
  ];

  const handleDismiss = () => {
    setIsVisible(false);
    localStorage.setItem('onboarding-tour-completed', 'true');
  };

  const handleNext = () => {
    if (step < steps.length - 1) {
      setStep(step + 1);
    } else {
      handleDismiss();
    }
  };

  const currentStep = steps[step];

  if (!isVisible) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-end p-4 pointer-events-none">
      <div className="bg-white rounded-lg shadow-xl p-6 max-w-sm pointer-events-auto animate-slide-up">
        <div className="flex items-start justify-between mb-4">
          <div className="flex-1">
            <h3 className="text-lg font-bold text-gray-900">{currentStep.title}</h3>
          </div>
          <button
            onClick={handleDismiss}
            className="text-gray-500 hover:text-gray-700 ml-2"
            aria-label="Close tour"
          >
            ✕
          </button>
        </div>

        <p className="text-gray-600 mb-6">{currentStep.description}</p>

        <div className="flex items-center justify-between">
          <div className="flex gap-1">
            {steps.map((_, i) => (
              <div
                key={i}
                className={`h-2 w-2 rounded-full ${i <= step ? 'bg-blue-600' : 'bg-gray-300'}`}
              />
            ))}
          </div>

          <div className="flex gap-2">
            <button
              onClick={handleDismiss}
              className="px-3 py-1 text-sm text-gray-600 hover:text-gray-900 font-medium"
            >
              Skip
            </button>
            <button
              onClick={handleNext}
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 text-sm font-medium"
            >
              {step === steps.length - 1 ? 'Done' : 'Next'}
            </button>
          </div>
        </div>
      </div>

      <style>{`
        @keyframes slide-up {
          from {
            opacity: 0;
            transform: translateY(20px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
        .animate-slide-up {
          animation: slide-up 0.3s ease-out;
        }
      `}</style>
    </div>
  );
}
